//! Converte file di macro nel formato .mc importato nei file `.macro` di Aurora (vedi `docs/macro-file.md`) e stampa un rapporto.
//!
//! ```text
//! cargo run -p mode-digi --example importa_mc -- <cartella di uscita> comuni=<file> cw=<file> rtty=<file> psk31=<file> psk63=<file>
//! ```
//!
//! Ogni coppia `gruppo=file` scrive `<gruppo>.macro` nella cartella di uscita. I comandi che Aurora conosce restano come sono (hanno gli stessi
//! nomi); il prefisso di codifica `UTF-8:` in testa al testo si toglie; i comandi sconosciuti restano nel testo e finiscono nel rapporto.
//! L'elenco dei comandi conosciuti si legge da `app/src/lib/macroLang.ts`, cosi' resta allineato con l'interprete.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mode_digi::macro_files;
use mode_digi::service::DigiMacro;

/// I gruppi validi: le macro comuni e i modi testuali.
const GROUPS: [&str; 5] = ["comuni", "psk31", "psk63", "rtty", "cw"];

/// I comandi conosciuti dall'interprete delle macro (`case 'XXX'` in macroLang.ts).
fn known_commands() -> Result<BTreeSet<String>, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../app/src/lib/macroLang.ts");
    let src = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = BTreeSet::new();
    for part in src.split("case '").skip(1) {
        if let Some(end) = part.find('\'') {
            out.insert(part[..end].to_string());
        }
    }
    Ok(out)
}

/// I comandi del testo (`<NOME>` o `<NOME:argomento>`), con il nome in maiuscolo.
fn commands(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let after = &rest[start + 1..];
        let Some(end) = after.find(['>', '<']) else { break };
        if after.as_bytes().get(end) == Some(&b'>') {
            let whole = &after[..end];
            let name = whole.split(':').next().unwrap_or("").trim().to_string();
            if !name.is_empty() {
                out.push((name, whole.to_string()));
            }
            rest = &after[end + 1..];
        } else {
            rest = &after[end..];
        }
    }
    out
}

/// Il testo tradotto: via il prefisso di codifica e la firma con la versione di un altro programma («Nome <VER> (Reg)» diventa «<VER>»).
fn translate(text: &str) -> String {
    let t = text.strip_prefix("UTF-8:").unwrap_or(text);
    let mut out = String::with_capacity(t.len());
    let mut rest = t;
    while let Some(at) = rest.find("<VER> (Reg)") {
        let before = &rest[..at];
        let word_start = before.trim_end().rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
        out.push_str(&before[..word_start]);
        out.push_str("<VER>");
        rest = &rest[at + "<VER> (Reg)".len()..];
    }
    out.push_str(rest);
    out
}

/// Legge un file come testo: UTF-8 se lo e', altrimenti Windows-1252 (latino).
fn read_text(path: &Path) -> Result<String, String> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(match String::from_utf8(b.clone()) {
        Ok(s) => s,
        Err(_) => b.iter().map(|&c| c as char).collect(),
    })
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out_dir = PathBuf::from(args.next().ok_or("manca la cartella di uscita")?);
    let known = known_commands()?;
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let mut unknown: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for arg in args {
        let (group, file) = arg.split_once('=').ok_or_else(|| format!("argomento non valido: {arg} (gruppo=file)"))?;
        if !GROUPS.contains(&group) {
            return Err(format!("gruppo sconosciuto: {group} (validi: {})", GROUPS.join(", ")));
        }
        let mut list: Vec<DigiMacro> = macro_files::parse(&read_text(Path::new(file))?);
        // per lo stesso tasto vale la prima voce (i file scritti a mano ripetono a volte un tasto)
        let mut names = BTreeSet::new();
        let before = list.len();
        list.retain(|m| names.insert(m.name.clone()));
        for m in &mut list {
            m.text = translate(&m.text);
            for (name, whole) in commands(&m.text) {
                if !known.contains(&name) {
                    unknown.entry(format!("<{whole}>")).or_default().insert(format!("{group}:{}", m.name));
                }
            }
        }
        let title = if group == "comuni" { "comuni a tutti i modi".to_string() } else { format!("solo {group}") };
        let path = out_dir.join(format!("{group}.{}", macro_files::EXTENSION));
        std::fs::write(&path, macro_files::render(&title, &list)).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{group}.macro: {} macro{}", list.len(), if before != list.len() { format!(" ({} doppioni saltati)", before - list.len()) } else { String::new() });
    }
    if unknown.is_empty() {
        println!("comandi non tradotti: nessuno");
    } else {
        println!("comandi non tradotti (restano nel testo):");
        for (cmd, wher) in &unknown {
            println!("  {cmd}  in {}", wher.iter().cloned().collect::<Vec<_>>().join(", "));
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("importa_mc: {e}");
        std::process::exit(1);
    }
}
