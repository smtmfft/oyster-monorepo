CREATE TABLE sync (
  key VARCHAR(16) PRIMARY KEY,
  value TEXT NOT NULL
);

-- Initial values
INSERT INTO sync VALUES (block, 0);
