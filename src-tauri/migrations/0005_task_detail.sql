-- Información completa de la tarea (texto plano, nunca HTML de Moodle)
ALTER TABLE assignment ADD COLUMN intro_text TEXT;
ALTER TABLE assignment ADD COLUMN intro_links_json TEXT;  -- [{name,url,file}]
