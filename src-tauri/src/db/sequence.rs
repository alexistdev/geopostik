//! Penomoran dokumen (`doc_sequences`), misal `OP-2609-0001`.

use rusqlite::Connection;

use crate::error::AppResult;

/// Nomor dokumen berikutnya `{prefix}-{period}-{urut 4 digit}`. Harus dipanggil di dalam transaksi
/// yang sama dengan insert dokumennya agar nomor tidak terpakai dua kali.
pub fn next_number(conn: &Connection, prefix: &str, period: &str) -> AppResult<String> {
    let no: i64 = conn.query_row(
        "INSERT INTO doc_sequences (prefix, period, last_no) VALUES (?1, ?2, 1)
         ON CONFLICT (prefix, period) DO UPDATE SET last_no = last_no + 1
         RETURNING last_no",
        [prefix, period],
        |r| r.get(0),
    )?;
    Ok(format!("{prefix}-{period}-{no:04}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_increase_per_period() {
        let conn = crate::db::open_in_memory().unwrap();
        assert_eq!(next_number(&conn, "OP", "2609").unwrap(), "OP-2609-0001");
        assert_eq!(next_number(&conn, "OP", "2609").unwrap(), "OP-2609-0002");
        assert_eq!(next_number(&conn, "OP", "2610").unwrap(), "OP-2610-0001");
        assert_eq!(next_number(&conn, "PJ", "2609").unwrap(), "PJ-2609-0001");
    }
}
