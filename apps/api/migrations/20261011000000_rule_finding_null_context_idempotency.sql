-- NULL context is the default-context scope and must share the same finding key.
ALTER TABLE rule_findings
    DROP CONSTRAINT rule_findings_workspace_id_rule_id_entity_id_context_id_eva_key;
ALTER TABLE rule_findings
    ADD CONSTRAINT rule_findings_workspace_id_rule_id_entity_id_context_id_eva_key
    UNIQUE NULLS NOT DISTINCT (workspace_id, rule_id, entity_id, context_id, evaluation_key);
