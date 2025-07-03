CREATE TABLE spaces (
    id TEXT PRIMARY KEY,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE pods (
    id TEXT NOT NULL,
    pod_type TEXT NOT NULL,
    data BLOB NOT NULL,
    label TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    space TEXT NOT NULL,
    PRIMARY KEY (space, id),
    FOREIGN KEY (space) REFERENCES spaces(id) ON DELETE CASCADE
); 