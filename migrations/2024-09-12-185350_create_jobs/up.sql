CREATE TABLE jobs (
  id CHAR(66) PRIMARY KEY,
  metadata text NOT NULL,
  owner CHAR(42) NOT NULL,
  provider CHAR(42) NOT NULL REFERENCES providers (id),
  rate BIGINT NOT NULL,
  balance BIGINT NOT NULL,
  last_settled timestamp NOT NULL,
  created timestamp NOT NULL
);

CREATE INDEX jobs_owner_idx ON jobs (owner);
CREATE INDEX jobs_provider_idx ON jobs (provider);
