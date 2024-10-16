CREATE TABLE transactions (
  block BIGINT NOT NULL,
  idx BIGINT NOT NULL,
  job CHAR(66) REFERENCES jobs (id),
  amount NUMERIC NOT NULL,
  is_deposit BOOL NOT NULL,
  PRIMARY KEY(block, idx)
);

CREATE INDEX transactions_block_idx_idx ON transactions(block, idx);
