//! Manifiesto del conjunto de evaluación (`eval/manifest.json`).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Digital,
    Scanned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
    pub license: String,
    pub kind: Kind,
    pub language: String,
    /// Qué pone a prueba este documento, en lenguaje llano.
    pub tests: String,
    /// Páginas que se miden (ambas incluidas, empezando en 1). Todas si falta.
    #[serde(default)]
    pub pages: Option<[u32; 2]>,
    /// Páginas con transcripción de referencia revisada en `eval/references/<id>/`.
    #[serde(default)]
    pub reference_pages: Vec<u32>,
    /// Si es privado, el archivo no se describe con URL pública y nunca se sube.
    #[serde(default)]
    pub private: bool,
}

impl Document {
    pub fn file_name(&self) -> String {
        format!("{}.pdf", self.id)
    }

    pub fn page_range(&self, page_count: u32) -> (u32, u32) {
        match self.pages {
            Some([first, last]) => (first.max(1), last.min(page_count)),
            None => (1, page_count),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub documents: Vec<Document>,
}

pub fn load(path: &Path) -> Result<Manifest, String> {
    let raw =
        fs::read_to_string(path).map_err(|e| format!("No se pudo leer el manifiesto: {e}"))?;
    let manifest: Manifest =
        serde_json::from_str(&raw).map_err(|e| format!("Manifiesto inválido: {e}"))?;
    if manifest.schema != 1 {
        return Err(format!(
            "Esquema de manifiesto no admitido: {}",
            manifest.schema
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for document in &manifest.documents {
        if !seen.insert(document.id.clone()) {
            return Err(format!("Identificador repetido: {}", document.id));
        }
        if document.sha256.len() != 64 || !document.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("SHA-256 inválido en {}", document.id));
        }
    }
    Ok(manifest)
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_rejects_duplicates_and_bad_hashes() {
        let dir = std::env::temp_dir().join(format!("liblevo-eval-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let doc = |id: &str, sha: &str| {
            format!(
                r#"{{"id":"{id}","title":"t","url":"u","sha256":"{sha}","bytes":1,"license":"l","kind":"digital","language":"en","tests":"x"}}"#
            )
        };
        let good = "a".repeat(64);
        let path = dir.join("m.json");
        fs::write(
            &path,
            format!(r#"{{"schema":1,"documents":[{}]}}"#, doc("x", &good)),
        )
        .unwrap();
        assert_eq!(load(&path).unwrap().documents.len(), 1);
        fs::write(
            &path,
            format!(
                r#"{{"schema":1,"documents":[{},{}]}}"#,
                doc("x", &good),
                doc("x", &good)
            ),
        )
        .unwrap();
        assert!(load(&path).unwrap_err().contains("repetido"));
        fs::write(
            &path,
            format!(r#"{{"schema":1,"documents":[{}]}}"#, doc("x", "zz")),
        )
        .unwrap();
        assert!(load(&path).unwrap_err().contains("SHA-256"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn page_range_is_clamped() {
        let document = Document {
            id: "x".into(),
            title: String::new(),
            url: String::new(),
            sha256: String::new(),
            bytes: 0,
            license: String::new(),
            kind: Kind::Digital,
            language: String::new(),
            tests: String::new(),
            pages: Some([0, 50]),
            reference_pages: vec![],
            private: false,
        };
        assert_eq!(document.page_range(10), (1, 10));
    }
}
