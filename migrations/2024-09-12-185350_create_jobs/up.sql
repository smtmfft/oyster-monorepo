CREATE TABLE jobs (
  id CHAR(66) PRIMARY KEY,
  metadata text NOT NULL,
  owner CHAR(42) NOT NULL,
  provider CHAR(42) NOT NULL REFERENCES providers (id),
  rate NUMERIC NOT NULL,
  balance NUMERIC NOT NULL,
  last_settled TIMESTAMP NOT NULL,
  created TIMESTAMP NOT NULL
);

CREATE INDEX jobs_owner_idx ON jobs (owner);
CREATE INDEX jobs_provider_idx ON jobs (provider);
