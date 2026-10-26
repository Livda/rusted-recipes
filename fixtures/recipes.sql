INSERT INTO app_recipe (diet, name, note, season, tags, url) VALUES
 ('veggie', 'Quiche aux poireaux', NULL, 'winter', '[]', 'http://localhost'),
 ('all', 'Chili con Carne', NULL, 'all', '[]', 'http://localhost'),
 ('all', 'Moussaka', 'Mettre une noix de muscade, bien rincer les aubergines, faire fondre les oignons avant de mettre les tomates.', 'all', '["aubergine","four","boeuf"]', 'http://localhost');
INSERT INTO app_recipe (diet, name, note, season, tags, url)
 WITH RECURSIVE seq (n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM seq WHERE n < 99)
 SELECT 'all', CONCAT('Toto ', n), NULL, 'all', '[]', 'http://localhost' FROM seq;
