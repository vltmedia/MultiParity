-- The look an organization or a project has, for people who have not picked their own: a theme
-- for light mode and one for dark mode (ids from the apps' theme list). NULL: inherit (a project
-- from its organization, an organization from the apps' defaults).
ALTER TABLE orgs ADD COLUMN theme_light TEXT, ADD COLUMN theme_dark TEXT;
ALTER TABLE projects ADD COLUMN theme_light TEXT, ADD COLUMN theme_dark TEXT;
