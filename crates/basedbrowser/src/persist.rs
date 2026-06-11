//! Persistência em disco do BasedBrowser (Marco M4, ADR-0007): favoritos, histórico de navegação e
//! sessão de abas. Tudo em JSON sob o diretório de config do usuário (`~/.config/basedbrowser/` no
//! Linux, resolvido via `dirs`). Princípios:
//!
//! - **Escrita atômica** (`tmp` + `rename`): um crash no meio da escrita não corrompe o arquivo bom.
//! - **Nunca paniqueia**: arquivo ausente ou JSON inválido = trata como vazio (com log), em vez de
//!   derrubar o browser. Erros de escrita são logados e engolidos (perder um favorito não vale um crash).
//! - **Sem `.unwrap()`/`.expect()`** em produção (lint `deny`); em testes é liberado (`clippy.toml`).
//!
//! A UI que consome isto (barra de favoritos, painel/autocomplete de histórico, restauração de sessão)
//! chega nas tarefas T5/T6/T7 do M4; aqui ficam o armazenamento e o estado em memória ([`AppData`]).

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Subdiretório do app dentro do diretório de config do usuário.
const APP_DIR: &str = "basedbrowser";
const BOOKMARKS_FILE: &str = "bookmarks.json";
const HISTORY_FILE: &str = "history.json";
const SESSION_FILE: &str = "session.json";

/// Teto de entradas do histórico em disco (FIFO: as mais antigas saem primeiro). Limita o tamanho do
/// arquivo sem virar um banco de dados — alinhado ao escopo do M4.
const HISTORY_CAP: usize = 1000;

/// Um favorito: rótulo exibido + URL de destino.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

/// Uma visita registrada no histórico (página visitada numa aba).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    /// Instante da visita em segundos desde a época Unix.
    pub visited_at: u64,
}

/// Sessão de abas persistida (URLs das abas abertas + índice da aba ativa). Restaurada no start (T7).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub tabs: Vec<String>,
    pub active: usize,
}

/// Estado de persistência vivo na memória durante a execução. Carregado no start ([`AppData::load`])
/// e gravado em disco conforme muda. A sessão NÃO mora aqui (é derivada das abas vivas no momento de
/// salvar — ver [`save_session`]).
#[derive(Debug, Default)]
pub struct AppData {
    pub bookmarks: Vec<Bookmark>,
    pub history: Vec<HistoryEntry>,
}

impl AppData {
    /// Carrega favoritos + histórico do disco (ou vazios, se ausentes/ inválidos).
    #[must_use]
    pub fn load() -> Self {
        Self {
            bookmarks: load_bookmarks(),
            history: load_history(),
        }
    }

    /// Registra uma visita no histórico em memória e persiste. Dedup consecutivo (recarregar a mesma
    /// URL só atualiza título/hora) e respeita o teto [`HISTORY_CAP`].
    pub fn record_visit(&mut self, url: &str, title: &str) {
        if !record_visit(&mut self.history, url, title) {
            return;
        }
        save_history(&self.history);
    }

    /// Limpa o histórico de navegação (memória + disco), gravando uma lista vazia (atômico). Parte do
    /// "limpar dados de navegação" do M6 (ADR-0009). Favoritos e sessão de abas NÃO são afetados —
    /// são curadoria do usuário, preservados por convenção de browser.
    pub fn clear_history(&mut self) {
        self.history.clear();
        save_history(&self.history);
    }
}

/// Diretório de config do app (`~/.config/basedbrowser/`). `None` se a plataforma não expõe um.
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|base| base.join(APP_DIR))
}

/// Diretório de dados persistentes do Servo (cookies + `localStorage`/`sessionStorage`), num subdir
/// próprio (`~/.config/basedbrowser/servo/`) p/ NÃO colidir com nossos `*.json`. É o que ligamos como
/// `Opts.config_dir` no `init_manager` (M6, ADR-0009) — honra `XDG_CONFIG_HOME` via `dirs`, preservando
/// os perfis-limpos do ADR-0008. Cria o diretório (best-effort; o Servo cria os arquivos dentro).
/// `None` se a plataforma não expõe diretório de config (aí o `init_manager` cai no default = sem
/// persistência, em vez de falhar).
#[must_use]
pub fn servo_config_dir() -> Option<PathBuf> {
    let dir = config_dir()?.join("servo");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!(
            "[m6] falha ao criar {} ({e}); persistência pode não funcionar",
            dir.display()
        );
    }
    Some(dir)
}

/// Carrega os favoritos (vazio se ausente/inválido).
#[must_use]
pub fn load_bookmarks() -> Vec<Bookmark> {
    config_dir()
        .map(|dir| read_json_or_default(&dir.join(BOOKMARKS_FILE)))
        .unwrap_or_default()
}

/// Persiste os favoritos (atômico). Erros são logados, não propagados.
pub fn save_bookmarks(bookmarks: &[Bookmark]) {
    save_under(BOOKMARKS_FILE, &bookmarks, "favoritos");
}

/// Carrega o histórico (vazio se ausente/inválido).
#[must_use]
pub fn load_history() -> Vec<HistoryEntry> {
    config_dir()
        .map(|dir| read_json_or_default(&dir.join(HISTORY_FILE)))
        .unwrap_or_default()
}

/// Persiste o histórico (atômico). Erros são logados, não propagados.
pub fn save_history(history: &[HistoryEntry]) {
    save_under(HISTORY_FILE, &history, "histórico");
}

/// Carrega a sessão de abas salva, se houver (`None` quando não há arquivo de sessão).
#[must_use]
pub fn load_session() -> Option<Session> {
    let path = config_dir()?.join(SESSION_FILE);
    if !path.exists() {
        return None;
    }
    Some(read_json_or_default(&path))
}

/// Persiste a sessão de abas (atômico). Erros são logados, não propagados.
pub fn save_session(session: &Session) {
    save_under(SESSION_FILE, session, "sessão");
}

/// Insere uma visita em `history`, devolvendo `true` se algo mudou (para o chamador decidir persistir).
/// Ignora URLs vazias; faz dedup consecutivo (mesma URL na ponta só atualiza título/hora) e aplica o
/// teto [`HISTORY_CAP`] descartando as entradas mais antigas.
pub fn record_visit(history: &mut Vec<HistoryEntry>, url: &str, title: &str) -> bool {
    if url.is_empty() {
        return false;
    }
    if let Some(last) = history.last_mut() {
        if last.url == url {
            title.clone_into(&mut last.title);
            last.visited_at = now_unix();
            return true;
        }
    }
    history.push(HistoryEntry {
        url: url.to_owned(),
        title: title.to_owned(),
        visited_at: now_unix(),
    });
    if history.len() > HISTORY_CAP {
        let excess = history.len() - HISTORY_CAP;
        history.drain(0..excess);
    }
    true
}

/// Segundos desde a época Unix (0 se o relógio estiver antes da época — não paniqueia).
#[must_use]
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Resolve `config_dir()/file` e grava `value` ali (atômico), logando o erro sob `label`. No-op se a
/// plataforma não expõe diretório de config.
fn save_under<T: Serialize + ?Sized>(file: &str, value: &T, label: &str) {
    let Some(dir) = config_dir() else {
        eprintln!("[m4] sem diretório de config; {label} não será salvo");
        return;
    };
    if let Err(e) = write_json_atomic(&dir.join(file), value) {
        eprintln!("[m4] falha ao salvar {label}: {e}");
    }
}

/// Lê e desserializa o JSON em `path`. Devolve `T::default()` (com log) se o arquivo não existir, não
/// puder ser lido, ou o JSON estiver inválido — o browser segue funcionando com estado vazio.
fn read_json_or_default<T: DeserializeOwned + Default>(path: &Path) -> T {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            eprintln!("[m4] {} inválido ({e}); usando vazio", path.display());
            T::default()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => T::default(),
        Err(e) => {
            eprintln!("[m4] falha ao ler {} ({e}); usando vazio", path.display());
            T::default()
        }
    }
}

/// Escreve `value` como JSON em `path` de forma **atômica**: serializa, grava num `path` + sufixo
/// `.tmp` e renomeia por cima (rename é atômico no mesmo sistema de arquivos). Cria o diretório pai se
/// necessário.
fn write_json_atomic<T: Serialize + ?Sized>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);

    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caminho de teste isolado (único por processo + nome), fora do `~/.config` real.
    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("bb-persist-{}-{}", std::process::id(), name))
    }

    #[test]
    fn roundtrip_bookmarks() {
        let path = temp_path("bookmarks.json");
        let _ = std::fs::remove_file(&path);
        let items = vec![
            Bookmark {
                title: "GitHub".into(),
                url: "https://github.com".into(),
            },
            Bookmark {
                title: "Servo".into(),
                url: "https://servo.org".into(),
            },
        ];
        write_json_atomic(&path, &items).unwrap();
        let back: Vec<Bookmark> = read_json_or_default(&path);
        assert_eq!(items, back);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_is_default_not_panic() {
        let path = temp_path("inexistente.json");
        let _ = std::fs::remove_file(&path);
        let back: Vec<HistoryEntry> = read_json_or_default(&path);
        assert!(back.is_empty());
    }

    #[test]
    fn corrupt_json_is_default_not_panic() {
        let path = temp_path("corrupto.json");
        std::fs::write(&path, b"{ isto nao e json valido ]").unwrap();
        let back: Vec<Bookmark> = read_json_or_default(&path);
        assert!(back.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn record_visit_dedups_consecutive_same_url() {
        let mut history = Vec::new();
        assert!(record_visit(&mut history, "https://a.com", "A"));
        assert!(record_visit(
            &mut history,
            "https://a.com",
            "A (recarregada)"
        ));
        assert_eq!(history.len(), 1, "mesma URL consecutiva não duplica");
        assert_eq!(history[0].title, "A (recarregada)");
        assert!(record_visit(&mut history, "https://b.com", "B"));
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn record_visit_ignores_empty_and_caps() {
        let mut history = Vec::new();
        assert!(!record_visit(&mut history, "", "vazia"));
        assert!(history.is_empty());
        for i in 0..(HISTORY_CAP + 50) {
            record_visit(&mut history, &format!("https://site/{i}"), "x");
        }
        assert_eq!(history.len(), HISTORY_CAP, "respeita o teto");
        assert_eq!(
            history[0].url,
            format!("https://site/{}", 50),
            "descarta as mais antigas (FIFO)"
        );
    }

    #[test]
    fn session_roundtrip() {
        let path = temp_path("session.json");
        let session = Session {
            tabs: vec!["https://a.com".into(), "https://b.com".into()],
            active: 1,
        };
        write_json_atomic(&path, &session).unwrap();
        let back: Session = read_json_or_default(&path);
        assert_eq!(session, back);
        let _ = std::fs::remove_file(&path);
    }
}
