-- Menu Sistem › Log: daftar audit diurutkan terbaru dan difilter per jenis data / user.

CREATE INDEX audit_logs_created_at ON audit_logs (created_at);
CREATE INDEX audit_logs_entity ON audit_logs (entity, entity_id);
CREATE INDEX audit_logs_user ON audit_logs (user_id);
