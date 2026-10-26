-- Schéma identique à celui de Doctrine (Version20211228174834) : `IF NOT EXISTS`
-- permet de pointer sur la base Symfony existante sans rien casser.
CREATE TABLE IF NOT EXISTS app_recipe (
    id INT AUTO_INCREMENT NOT NULL,
    diet VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL,
    note VARCHAR(255) DEFAULT NULL,
    season VARCHAR(255) NOT NULL,
    tags LONGTEXT NOT NULL COMMENT '(DC2Type:json)',
    url LONGTEXT NOT NULL,
    PRIMARY KEY (id)
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS app_user (
    id INT AUTO_INCREMENT NOT NULL,
    username VARCHAR(180) NOT NULL,
    roles LONGTEXT NOT NULL COMMENT '(DC2Type:json)',
    password VARCHAR(255) NOT NULL,
    UNIQUE INDEX UNIQ_88BDF3E9F85E0677 (username),
    PRIMARY KEY (id)
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;
