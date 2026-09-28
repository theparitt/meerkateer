-- A service is one game instance, independent of the machine that hosts it.
-- NULL fields preserve existing application services and make game monitoring opt-in.
ALTER TABLE services
    ADD COLUMN game_kind text,
    ADD COLUMN game_host text,
    ADD COLUMN game_port integer,
    ADD CONSTRAINT services_game_config_complete CHECK (
        (game_kind IS NULL AND game_host IS NULL AND game_port IS NULL)
        OR (game_kind = 'minecraft_java'
            AND game_host IS NOT NULL
            AND game_port IS NOT NULL
            AND char_length(game_host) BETWEEN 1 AND 253
            AND game_port BETWEEN 1 AND 65535)
    );
