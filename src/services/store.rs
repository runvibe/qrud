use sqlite::{Connection, State as SqlState};

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &str) -> Result<Self, String> {
        let conn = sqlite::open(path).map_err(|err| err.to_string())?;
        init_db(&conn)?;
        Ok(Self { conn })
    }

    pub fn next_id_for(&mut self, collection: &str) -> Result<i64, String> {
        let mut statement = self
            .conn
            .prepare("SELECT next_id FROM counters WHERE collection = ?;")
            .map_err(|err| err.to_string())?;
        statement
            .bind((1, collection))
            .map_err(|err| err.to_string())?;

        let next_id = if let SqlState::Row = statement.next().map_err(|err| err.to_string())? {
            statement
                .read::<i64, _>(0)
                .map_err(|err| err.to_string())?
        } else {
            max_id_for(&self.conn, collection)? + 1
        };

        let mut update = self
            .conn
            .prepare(
                "INSERT INTO counters (collection, next_id)
                 VALUES (?, ?)
                 ON CONFLICT(collection) DO UPDATE SET next_id = excluded.next_id;",
            )
            .map_err(|err| err.to_string())?;
        update
            .bind((1, collection))
            .map_err(|err| err.to_string())?;
        update
            .bind((2, next_id + 1))
            .map_err(|err| err.to_string())?;
        update.next().map_err(|err| err.to_string())?;

        Ok(next_id)
    }

    pub fn bump_next_id(&mut self, collection: &str, used_id: i64) -> Result<(), String> {
        let mut statement = self
            .conn
            .prepare("SELECT next_id FROM counters WHERE collection = ?;")
            .map_err(|err| err.to_string())?;
        statement
            .bind((1, collection))
            .map_err(|err| err.to_string())?;

        let current_next = if let SqlState::Row = statement.next().map_err(|err| err.to_string())? {
            statement
                .read::<i64, _>(0)
                .map_err(|err| err.to_string())?
        } else {
            max_id_for(&self.conn, collection)? + 1
        };

        let desired_next = (used_id + 1).max(current_next);
        let mut update = self
            .conn
            .prepare(
                "INSERT INTO counters (collection, next_id)
                 VALUES (?, ?)
                 ON CONFLICT(collection) DO UPDATE SET next_id = excluded.next_id;",
            )
            .map_err(|err| err.to_string())?;
        update
            .bind((1, collection))
            .map_err(|err| err.to_string())?;
        update
            .bind((2, desired_next))
            .map_err(|err| err.to_string())?;
        update.next().map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn insert_item(&mut self, collection: &str, id: i64, data: &str) -> Result<(), String> {
        let mut statement = self
            .conn
            .prepare("INSERT INTO items (collection, id, data) VALUES (?, ?, ?);")
            .map_err(|err| err.to_string())?;
        statement
            .bind((1, collection))
            .map_err(|err| err.to_string())?;
        statement.bind((2, id)).map_err(|err| err.to_string())?;
        statement
            .bind((3, data))
            .map_err(|err| err.to_string())?;
        statement.next().map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn upsert_item(
        &mut self,
        collection: &str,
        id: i64,
        data: &str,
    ) -> Result<(), String> {
        let mut statement = self
            .conn
            .prepare(
                "INSERT INTO items (collection, id, data)
                 VALUES (?, ?, ?)
                 ON CONFLICT(collection, id) DO UPDATE SET data = excluded.data;",
            )
            .map_err(|err| err.to_string())?;
        statement
            .bind((1, collection))
            .map_err(|err| err.to_string())?;
        statement.bind((2, id)).map_err(|err| err.to_string())?;
        statement
            .bind((3, data))
            .map_err(|err| err.to_string())?;
        statement.next().map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn update_item(&mut self, collection: &str, id: i64, data: &str) -> Result<(), String> {
        let mut statement = self
            .conn
            .prepare("UPDATE items SET data = ? WHERE collection = ? AND id = ?;")
            .map_err(|err| err.to_string())?;
        statement
            .bind((1, data))
            .map_err(|err| err.to_string())?;
        statement
            .bind((2, collection))
            .map_err(|err| err.to_string())?;
        statement.bind((3, id)).map_err(|err| err.to_string())?;
        statement.next().map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn item_exists(&mut self, collection: &str, id: i64) -> Result<bool, String> {
        let mut statement = self
            .conn
            .prepare("SELECT 1 FROM items WHERE collection = ? AND id = ? LIMIT 1;")
            .map_err(|err| err.to_string())?;
        statement
            .bind((1, collection))
            .map_err(|err| err.to_string())?;
        statement.bind((2, id)).map_err(|err| err.to_string())?;
        Ok(matches!(
            statement.next().map_err(|err| err.to_string())?,
            SqlState::Row
        ))
    }

    pub fn fetch_item_data(
        &mut self,
        collection: &str,
        id: i64,
    ) -> Result<Option<String>, String> {
        let mut statement = self
            .conn
            .prepare("SELECT data FROM items WHERE collection = ? AND id = ?;")
            .map_err(|err| err.to_string())?;
        statement
            .bind((1, collection))
            .map_err(|err| err.to_string())?;
        statement.bind((2, id)).map_err(|err| err.to_string())?;
        if let SqlState::Row = statement.next().map_err(|err| err.to_string())? {
            let data = statement
                .read::<String, _>(0)
                .map_err(|err| err.to_string())?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    pub fn delete_item(&mut self, collection: &str, id: i64) -> Result<bool, String> {
        if !self.item_exists(collection, id)? {
            return Ok(false);
        }
        let mut statement = self
            .conn
            .prepare("DELETE FROM items WHERE collection = ? AND id = ?;")
            .map_err(|err| err.to_string())?;
        statement
            .bind((1, collection))
            .map_err(|err| err.to_string())?;
        statement.bind((2, id)).map_err(|err| err.to_string())?;
        statement.next().map_err(|err| err.to_string())?;
        Ok(true)
    }

    pub fn list_collection(&mut self, collection: &str) -> Result<Vec<String>, String> {
        let mut statement = self
            .conn
            .prepare("SELECT data FROM items WHERE collection = ? ORDER BY id ASC;")
            .map_err(|err| err.to_string())?;
        statement
            .bind((1, collection))
            .map_err(|err| err.to_string())?;

        let mut rows = Vec::new();
        while let SqlState::Row = statement.next().map_err(|err| err.to_string())? {
            let data = statement
                .read::<String, _>(0)
                .map_err(|err| err.to_string())?;
            rows.push(data);
        }
        Ok(rows)
    }
}

fn init_db(connection: &Connection) -> Result<(), String> {
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS items (
                collection TEXT NOT NULL,
                id INTEGER NOT NULL,
                data TEXT NOT NULL,
                PRIMARY KEY (collection, id)
            );",
        )
        .map_err(|err| err.to_string())?;
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS counters (
                collection TEXT PRIMARY KEY,
                next_id INTEGER NOT NULL
            );",
        )
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn max_id_for(conn: &Connection, collection: &str) -> Result<i64, String> {
    let mut statement = conn
        .prepare("SELECT COALESCE(MAX(id), 0) FROM items WHERE collection = ?;")
        .map_err(|err| err.to_string())?;
    statement
        .bind((1, collection))
        .map_err(|err| err.to_string())?;
    if let SqlState::Row = statement.next().map_err(|err| err.to_string())? {
        statement
            .read::<i64, _>(0)
            .map_err(|err| err.to_string())
    } else {
        Ok(0)
    }
}
