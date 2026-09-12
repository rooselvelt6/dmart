DEFINE INDEX IF NOT EXISTS idx_users_user_id ON users COLUMNS user_id;
DEFINE INDEX IF NOT EXISTS idx_mfa_settings_enabled ON mfa_settings COLUMNS enabled;