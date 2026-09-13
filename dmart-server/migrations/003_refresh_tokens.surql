-- SPEC-004: refresh tokens rotativos (single-use) con detección de reuso.
-- `token_hash` es SHA-256 del token (alta entropía: 256 bits aleatorios), lo
-- que permite un lookup indexado O(1). Argon2id se reserva para passwords de
-- baja entropía (ver auth.rs). Los registros nunca se borran en caliente: se
-- marcan `revoked = true` para poder detectar la reutilización de un token ya
-- rotado (posible robo) y revocar toda la familia de sesiones del usuario.

DEFINE TABLE IF NOT EXISTS refresh_token SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS token_hash ON refresh_token TYPE string;
DEFINE FIELD IF NOT EXISTS user_id ON refresh_token TYPE string;
DEFINE FIELD IF NOT EXISTS expires_at ON refresh_token TYPE int;
DEFINE FIELD IF NOT EXISTS revoked ON refresh_token TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS created_at ON refresh_token TYPE int;
DEFINE FIELD IF NOT EXISTS user_agent ON refresh_token TYPE option<string>;
DEFINE FIELD IF NOT EXISTS ip_address ON refresh_token TYPE option<string>;
DEFINE INDEX IF NOT EXISTS idx_refresh_token_hash ON refresh_token COLUMNS token_hash UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_refresh_token_user ON refresh_token COLUMNS user_id;
