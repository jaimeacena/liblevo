//! Estado durable de la app en SQLite: los documentos añadidos y sus opciones.
//!
//! La app es la única que escribe en esta base. Cada cambio es una transacción; un original que
//! cambió o desapareció se detecta al reabrir comparando su huella.

use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 1: documentos. 2: opción de lectura visual completa por documento. 3: decisiones de revisión.
pub const SCHEMA_VERSION: i64 = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct StoredDocument {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub pages: u32,
    /// Releer todas las páginas del escaneo con el modelo visual (lento).
    pub visual_full: bool,
}

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            // Una base creada por una versión posterior no se toca.
            return Err(rusqlite::Error::InvalidQuery);
        }
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS documents (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 path TEXT NOT NULL,
                 pages INTEGER NOT NULL,
                 added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
             );",
        )?;
        let has_visual_full = connection
            .prepare("SELECT 1 FROM pragma_table_info('documents') WHERE name = 'visual_full'")?
            .exists([])?;
        // Migración 2 → 3: decisiones de revisión por fragmento (se borran con el documento).
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS review_decisions (
                 document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                 block_key TEXT NOT NULL,
                 decision TEXT NOT NULL,
                 decided_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                 PRIMARY KEY (document_id, block_key)
             );",
        )?;
        if !has_visual_full {
            // Migración 1 → 2: la opción empieza desactivada en los documentos ya añadidos.
            connection.execute_batch(
                "ALTER TABLE documents ADD COLUMN visual_full INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        connection.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self { connection })
    }

    pub fn add(&self, document: &StoredDocument) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT INTO documents (id, name, path, pages) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, path = excluded.path",
            params![
                document.id,
                document.name,
                document.path.to_string_lossy(),
                document.pages
            ],
        )?;
        Ok(())
    }

    pub fn set_visual_full(&self, id: &str, on: bool) -> rusqlite::Result<bool> {
        Ok(self.connection.execute(
            "UPDATE documents SET visual_full = ?2 WHERE id = ?1",
            params![id, on],
        )? > 0)
    }

    /// Guarda la decisión sobre un fragmento («correcto», «corregido» o «despues»).
    pub fn decide(&self, id: &str, key: &str, decision: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT INTO review_decisions (document_id, block_key, decision) VALUES (?1, ?2, ?3)
             ON CONFLICT(document_id, block_key) DO UPDATE SET decision = excluded.decision,
             decided_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
            params![id, key, decision],
        )?;
        Ok(())
    }

    /// Decisiones de revisión de un documento: huella del fragmento → decisión.
    pub fn decisions(&self, id: &str) -> rusqlite::Result<HashMap<String, String>> {
        let mut statement = self
            .connection
            .prepare("SELECT block_key, decision FROM review_decisions WHERE document_id = ?1")?;
        let rows = statement.query_map(params![id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    /// Una nueva conversión cambia los fragmentos: las decisiones anteriores dejan de aplicar.
    pub fn clear_decisions(&self, id: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "DELETE FROM review_decisions WHERE document_id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn remove(&self, id: &str) -> rusqlite::Result<bool> {
        Ok(self
            .connection
            .execute("DELETE FROM documents WHERE id = ?1", params![id])?
            > 0)
    }

    pub fn list(&self) -> rusqlite::Result<Vec<StoredDocument>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, path, pages, visual_full FROM documents ORDER BY added_at, rowid",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(StoredDocument {
                id: row.get(0)?,
                name: row.get(1)?,
                path: PathBuf::from(row.get::<_, String>(2)?),
                pages: row.get(3)?,
                visual_full: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn get(&self, id: &str) -> rusqlite::Result<Option<StoredDocument>> {
        Ok(self.list()?.into_iter().find(|d| d.id == id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(id: &str) -> StoredDocument {
        StoredDocument {
            id: id.into(),
            name: format!("{id}.pdf"),
            path: PathBuf::from(format!("C:/docs/{id}.pdf")),
            pages: 3,
            visual_full: false,
        }
    }

    #[test]
    fn documents_survive_reopening_in_order() {
        let dir = std::env::temp_dir().join(format!("liblevo-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("workspace.sqlite3");
        {
            let store = Store::open(&path).unwrap();
            store.add(&document("b")).unwrap();
            store.add(&document("a")).unwrap();
            store.add(&document("b")).unwrap(); // repetido: no duplica
        }
        let store = Store::open(&path).unwrap();
        let ids: Vec<String> = store.list().unwrap().into_iter().map(|d| d.id).collect();
        assert_eq!(ids, vec!["b", "a"]);
        assert!(store.remove("b").unwrap());
        assert!(!store.remove("b").unwrap());
        assert_eq!(store.get("a").unwrap().unwrap().pages, 3);
        assert!(store.set_visual_full("a", true).unwrap());
        assert!(store.get("a").unwrap().unwrap().visual_full);
        store.decide("a", "k1", "correcto").unwrap();
        store.decide("a", "k1", "corregido").unwrap();
        assert_eq!(store.decisions("a").unwrap()["k1"], "corregido");
        store.clear_decisions("a").unwrap();
        assert!(store.decisions("a").unwrap().is_empty());
        // Volver a añadir el mismo PDF no borra su opción.
        store.add(&document("a")).unwrap();
        assert!(store.get("a").unwrap().unwrap().visual_full);
        drop(store);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_version_one_database_is_migrated() {
        let dir = std::env::temp_dir().join(format!("liblevo-store-v1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("workspace.sqlite3");
        {
            let old = Connection::open(&path).unwrap();
            old.execute_batch(
                "CREATE TABLE documents (id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL,
                 pages INTEGER NOT NULL, added_at TEXT NOT NULL DEFAULT 'x');
                 INSERT INTO documents (id, name, path, pages) VALUES ('v', 'v.pdf', 'C:/v.pdf', 7);
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        }
        let store = Store::open(&path).unwrap();
        let migrated = store.get("v").unwrap().unwrap();
        assert_eq!((migrated.pages, migrated.visual_full), (7, false));
        drop(store);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_a_newer_schema() {
        let dir = std::env::temp_dir().join(format!("liblevo-store-new-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("workspace.sqlite3");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        assert!(Store::open(&path).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
