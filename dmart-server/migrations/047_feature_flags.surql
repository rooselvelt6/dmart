-- Migration 047: Feature Flags table
-- SPEC-047: Feature Flags por tenant

DEFINE TABLE feature_flag SCHEMAFULL;
DEFINE FIELD key ON feature_flag TYPE string;
DEFINE FIELD description ON feature_flag TYPE string;
DEFINE FIELD enabled ON feature_flag TYPE bool;
DEFINE FIELD tenant_id ON feature_flag TYPE option<string>;
DEFINE FIELD created_at ON feature_flag TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON feature_flag TYPE datetime DEFAULT time::now();

DEFINE INDEX idx_feature_flag_key_tenant ON feature_flag COLUMNS key, tenant_id UNIQUE;
DEFINE INDEX idx_feature_flag_tenant ON feature_flag COLUMNS tenant_id;