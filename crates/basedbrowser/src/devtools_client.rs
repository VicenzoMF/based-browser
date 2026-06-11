//! Cliente de devtools (M7, ADR-0010): conecta no servidor de devtools do PRÓPRIO Servo (loopback) e
//! extrai os eventos de REDE para o painel in-app — sem Firefox externo. É a única forma de ver o lado
//! da RESPOSTA (status/headers/payload), que o embedder NÃO vê pelos hooks normais (parede do M6/L-009):
//! o dado existe, mas o crate `servo-devtools` é hermético (só `start_server`) e os eventos só saem por
//! este socket TCP falando o protocolo de remote-debugging do Firefox (RDP).
//!
//! **Por que não é frágil como o Verso temia:** os dois lados são NOSSOS, na mesma `servo 0.2.0` pinada
//! (ADR-0002) — o caveat "só testado com Firefox nightly" do upstream não se aplica, e o protocolo fica
//! fixo pelo pin (churn tratado nos sprints de update, como todo o resto).
//!
//! **Wire** (confirmado empiricamente + na fonte do `servo-devtools-0.2.0`): mensagens são
//! `<byte-len>:<json>`. Handshake mínimo p/ assinar eventos de rede — o servidor envia o ator `root`;
//! o cliente manda `listTabs` (→ `tabs[0].actor`), depois `getWatcher` ao ator da aba (reply traz o
//! watcher no campo `actor`, flatten), depois `watchResources` com `resourceTypes:["network-event"]`.
//! Daí chegam `resources-available-array` (lado da requisição: `actor`/`method`/`url`/`resourceId`) e
//! `resources-updated-array` (`resourceUpdates.status` = `"200 OK"`, flags `*Available`).
//! Headers/payload reais vêm sob demanda do `NetworkEventActor`: `getRequestHeaders`/
//! `getResponseHeaders` (`{headers:[{name,value}],rawHeaders}`) e `getResponseContent`
//! (`{content:{text,mimeType,...}}`).
//!
//! **Threading (ADR-0007):** roda numa thread dedicada e SÓ comunica com a UI por um canal — o LOOP
//! drena os `NetRecord` e escreve nos models do Slint. Nunca toca a UI a partir desta thread. Nunca
//! paniqueia (lints `deny`): erros de IO/parse encerram a thread limpa e o painel fica como está.

use std::collections::HashMap;
use std::io::{self, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use std::time::Duration;

use serde_json::{json, Value};

/// Um evento de rede (request + response) acumulado pelo cliente e enviado à UI por snapshot a cada
/// atualização (a UI faz upsert por `id`). `Clone` p/ atravessar o canal.
#[derive(Clone, Default)]
pub struct NetRecord {
    pub id: u64,
    pub method: String,
    pub url: String,
    /// `"200 OK"` quando a resposta chega; vazio enquanto pendente.
    pub status: String,
    pub mime: String,
    pub req_headers: Vec<(String, String)>,
    pub resp_headers: Vec<(String, String)>,
    /// Corpo da resposta (texto), truncado em [`BODY_CAP`].
    pub resp_body: String,
}

/// Teto do corpo de resposta guardado/exibido (evita estourar memória num download grande).
const BODY_CAP: usize = 4096;

/// Spawna a thread do cliente RDP de rede. Fire-and-forget: encerra sozinha quando o socket fecha (app
/// saindo) ou em erro. Loga o desfecho. Reusa `std::thread`/`std::net` — nenhuma dep nova.
pub fn spawn(port: u16, tx: Sender<NetRecord>) {
    if let Err(e) = std::thread::Builder::new()
        .name("devtools-net".into())
        .spawn(move || match run(port, &tx) {
            Ok(()) => eprintln!("[m7] devtools-net: conexão encerrada"),
            Err(e) => eprintln!("[m7] devtools-net: encerrado ({e})"),
        })
    {
        eprintln!("[m7] devtools-net: falha ao spawnar a thread: {e}");
    }
}

fn run(port: u16, tx: &Sender<NetRecord>) -> io::Result<()> {
    let stream = TcpStream::connect(("127.0.0.1", port))?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);

    // 1) Ator `root` (esperamos; o servidor faz peek-timeout do token e então o embedder autoriza).
    let _root = read_packet(&mut reader)?;
    // 2) listTabs → ator da 1ª aba. RETRY: o cliente sobe cedo (notify_devtools_server_started) e a aba
    //    pode ainda não estar registrada no devtools → `tabs:[]`. Repetimos até aparecer uma aba (sem
    //    isso o handshake travaria esperando um `tabs[0]` que nunca vem — corrida de timing).
    let tab_actor = loop {
        write_packet(&mut writer, &json!({ "to": "root", "type": "listTabs" }))?;
        let reply = next_matching(&mut reader, |v| v.get("tabs").map(|_| v.clone()))?;
        let actor = reply
            .get("tabs")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(|t| t.get("actor"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let Some(actor) = actor {
            break actor;
        }
        std::thread::sleep(Duration::from_millis(300));
    };
    // 3) getWatcher → o watcher vem no campo `actor` (flatten) do reply cujo `from` é a aba.
    write_packet(
        &mut writer,
        &json!({ "to": tab_actor, "type": "getWatcher" }),
    )?;
    let watcher = next_matching(&mut reader, |v| {
        if v.get("from").and_then(Value::as_str) == Some(tab_actor.as_str()) {
            v.get("actor").and_then(Value::as_str).map(str::to_owned)
        } else {
            None
        }
    })?;
    // 4) Assina os eventos de rede (só FUTUROS — não há snapshot p/ network-event; por isso o cliente
    //    sobe cedo, no notify_devtools_server_started, e captura as requisições da página).
    write_packet(
        &mut writer,
        &json!({
            "to": watcher,
            "type": "watchResources",
            "resourceTypes": ["network-event"],
        }),
    )?;
    eprintln!("[m7] devtools-net: assinado a eventos de rede (watcher {watcher})");

    let mut client = NetClient::default();
    loop {
        let packet = read_packet(&mut reader)?;
        client.handle(&packet, &mut writer, tx)?;
    }
}

/// Estado do cliente: mapeia `NetworkEventActor` ↔ `resourceId` e acumula os `NetRecord` por id.
#[derive(Default)]
struct NetClient {
    id_by_actor: HashMap<String, u64>,
    actor_by_id: HashMap<u64, String>,
    records: HashMap<u64, NetRecord>,
    fetched: HashMap<u64, bool>,
    /// Por ator: quantos replies de `headers` já vieram (0 → request, 1 → response), já que pedimos
    /// `getRequestHeaders` antes de `getResponseHeaders` e os replies do MESMO ator são ordenados.
    header_phase: HashMap<String, u8>,
}

impl NetClient {
    fn handle<W: Write>(
        &mut self,
        packet: &Value,
        writer: &mut W,
        tx: &Sender<NetRecord>,
    ) -> io::Result<()> {
        let typ = packet.get("type").and_then(Value::as_str).unwrap_or("");
        match typ {
            "resources-available-array" | "resources-updated-array" => {
                for id in
                    self.ingest_resources(packet, typ == "resources-available-array", writer)?
                {
                    self.emit(id, tx);
                }
            }
            _ => {
                // Possível reply do NetworkEventActor (headers/content).
                if let Some(from) = packet.get("from").and_then(Value::as_str) {
                    if let Some(&id) = self.id_by_actor.get(from) {
                        self.ingest_reply(from, id, packet);
                        self.emit(id, tx);
                    }
                }
            }
        }
        Ok(())
    }

    /// Processa `resources-{available,updated}-array`, devolvendo os ids tocados. Pode disparar os
    /// pedidos de headers/content quando a resposta fica disponível.
    fn ingest_resources<W: Write>(
        &mut self,
        packet: &Value,
        available: bool,
        writer: &mut W,
    ) -> io::Result<Vec<u64>> {
        let mut touched = Vec::new();
        let Some(array) = packet.get("array").and_then(Value::as_array) else {
            return Ok(touched);
        };
        for entry in array {
            let Some(pair) = entry.as_array() else {
                continue;
            };
            if pair.first().and_then(Value::as_str) != Some("network-event") {
                continue;
            }
            let Some(items) = pair.get(1).and_then(Value::as_array) else {
                continue;
            };
            for item in items {
                let Some(id) = item.get("resourceId").and_then(Value::as_u64) else {
                    continue;
                };
                if available {
                    self.ingest_available(id, item);
                } else if let Some(actor) = self.ingest_updated(id, item) {
                    // Resposta disponível e ainda não buscamos os detalhes → pede headers + content.
                    if !self.fetched.get(&id).copied().unwrap_or(false) {
                        self.fetched.insert(id, true);
                        self.header_phase.insert(actor.clone(), 0);
                        request_details(writer, &actor)?;
                    }
                }
                touched.push(id);
            }
        }
        touched.sort_unstable();
        touched.dedup();
        Ok(touched)
    }

    fn ingest_available(&mut self, id: u64, item: &Value) {
        let rec = self.records.entry(id).or_default();
        rec.id = id;
        if let Some(m) = item.get("method").and_then(Value::as_str) {
            m.clone_into(&mut rec.method);
        }
        if let Some(u) = item.get("url").and_then(Value::as_str) {
            u.clone_into(&mut rec.url);
        }
        if let Some(actor) = item.get("actor").and_then(Value::as_str) {
            self.id_by_actor.insert(actor.to_owned(), id);
            self.actor_by_id.insert(id, actor.to_owned());
        }
    }

    /// Aplica um `resourceUpdates`; devolve o ator se a RESPOSTA ficou disponível (status presente).
    fn ingest_updated(&mut self, id: u64, item: &Value) -> Option<String> {
        let updates = item.get("resourceUpdates")?;
        let rec = self.records.entry(id).or_default();
        rec.id = id;
        let mut response_ready = false;
        if let Some(status) = updates.get("status").and_then(Value::as_str) {
            status.clone_into(&mut rec.status);
            response_ready = true;
        }
        if updates
            .get("responseHeadersAvailable")
            .and_then(Value::as_bool)
            == Some(true)
        {
            response_ready = true;
        }
        if response_ready {
            self.actor_by_id.get(&id).cloned()
        } else {
            None
        }
    }

    fn ingest_reply(&mut self, actor: &str, id: u64, packet: &Value) {
        let rec = self.records.entry(id).or_default();
        rec.id = id;
        if let Some(content) = packet.get("content") {
            if let Some(mime) = content.get("mimeType").and_then(Value::as_str) {
                mime.clone_into(&mut rec.mime);
            }
            rec.resp_body = truncate(&value_to_text(content.get("text")));
        } else if let Some(headers) = packet.get("headers").and_then(Value::as_array) {
            let parsed = parse_headers(headers);
            // 1º reply de headers do ator = request; 2º = response (pedimos nessa ordem).
            let phase = self.header_phase.entry(actor.to_owned()).or_insert(0);
            if *phase == 0 {
                rec.req_headers = parsed;
                *phase = 1;
            } else {
                rec.resp_headers = parsed;
            }
        }
    }

    fn emit(&self, id: u64, tx: &Sender<NetRecord>) {
        if let Some(rec) = self.records.get(&id) {
            // Erro de send = UI saiu; ignoramos (a thread morre no próximo read).
            let _ = tx.send(rec.clone());
        }
    }
}

/// Pede os detalhes (headers de request/response + corpo) de um `NetworkEventActor`. A ordem importa:
/// request-headers antes de response-headers (ver `header_phase`).
fn request_details<W: Write>(writer: &mut W, actor: &str) -> io::Result<()> {
    write_packet(writer, &json!({ "to": actor, "type": "getRequestHeaders" }))?;
    write_packet(
        writer,
        &json!({ "to": actor, "type": "getResponseHeaders" }),
    )?;
    write_packet(
        writer,
        &json!({ "to": actor, "type": "getResponseContent" }),
    )?;
    Ok(())
}

fn parse_headers(headers: &[Value]) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|h| {
            let name = h.get("name").and_then(Value::as_str)?;
            let value = h.get("value").and_then(Value::as_str).unwrap_or("");
            Some((name.to_owned(), value.to_owned()))
        })
        .collect()
}

/// O `content.text` pode ser uma string ou (raro) outro valor JSON; normaliza p/ texto.
fn value_to_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn truncate(s: &str) -> String {
    if s.len() <= BODY_CAP {
        s.to_owned()
    } else {
        let mut end = BODY_CAP;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

/// Lê uma mensagem `<byte-len>:<json>` do stream e a parseia. Usa `BufReader` (o len vem dígito a
/// dígito). EOF/len inválido viram erro → encerra a thread.
fn read_packet<R: Read>(reader: &mut R) -> io::Result<Value> {
    let mut len_bytes = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if reader.read(&mut byte)? == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "eof no len"));
        }
        if byte[0] == b':' {
            break;
        }
        if byte[0].is_ascii_digit() {
            len_bytes.push(byte[0]);
        }
    }
    let len: usize = std::str::from_utf8(&len_bytes)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "len inválido"))?;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    serde_json::from_slice(&payload).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Escreve uma mensagem `<byte-len>:<json>`.
fn write_packet<W: Write>(writer: &mut W, value: &Value) -> io::Result<()> {
    let body = value.to_string();
    write!(writer, "{}:{}", body.len(), body)?;
    writer.flush()
}

/// Lê pacotes até o predicado extrair um valor (pacotes não-relacionados interleaveiam o stream).
fn next_matching<R: Read, T>(
    reader: &mut R,
    mut f: impl FnMut(&Value) -> Option<T>,
) -> io::Result<T> {
    loop {
        let packet = read_packet(reader)?;
        if let Some(value) = f(&packet) {
            return Ok(value);
        }
    }
}
