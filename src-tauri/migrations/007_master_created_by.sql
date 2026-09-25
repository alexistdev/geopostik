-- Kategori, rak, dan pabrik mencatat user yang membuatnya, untuk ditampilkan di menu Master Data.
-- Data yang dibuat sebelum migration ini tidak diketahui pembuatnya (NULL).

ALTER TABLE categories ADD COLUMN created_by INTEGER REFERENCES users (id);
ALTER TABLE racks ADD COLUMN created_by INTEGER REFERENCES users (id);
ALTER TABLE manufacturers ADD COLUMN created_by INTEGER REFERENCES users (id);
