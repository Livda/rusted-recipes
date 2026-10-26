#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diet {
    All,
    Veggie,
    Vegan,
}

impl Diet {
    pub const ALL: [Diet; 3] = [Diet::All, Diet::Veggie, Diet::Vegan];

    pub fn as_str(self) -> &'static str {
        match self {
            Diet::All => "all",
            Diet::Veggie => "veggie",
            Diet::Vegan => "vegan",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Diet::All => "Tout régime",
            Diet::Veggie => "Veggie",
            Diet::Vegan => "Vegan",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    All,
    Winter,
    Spring,
    Summer,
    Autumn,
}

impl Season {
    pub const ALL: [Season; 5] = [
        Season::All,
        Season::Winter,
        Season::Spring,
        Season::Summer,
        Season::Autumn,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Season::All => "all",
            Season::Winter => "winter",
            Season::Spring => "spring",
            Season::Summer => "summer",
            Season::Autumn => "autumn",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Season::All => "Toute saison",
            Season::Winter => "Hiver",
            Season::Spring => "Printemps",
            Season::Summer => "Été",
            Season::Autumn => "Automne",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub id: i32,
    pub diet: Diet,
    pub name: String,
    pub note: Option<String>,
    pub season: Season,
    pub tags: Vec<String>,
    pub url: String,
}

/// Données validées d'une recette, prêtes à être écrites.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewRecipe {
    pub diet: Diet,
    pub name: String,
    pub note: Option<String>,
    pub season: Season,
    pub tags: Vec<String>,
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub roles: Vec<String>,
    pub password: String,
}

impl User {
    /// Comme Symfony : tout utilisateur a au moins `ROLE_USER`.
    pub fn has_role(&self, role: &str) -> bool {
        role == "ROLE_USER" || self.roles.iter().any(|r| r == role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diet_round_trip() {
        for diet in Diet::ALL {
            assert_eq!(Diet::parse(diet.as_str()), Some(diet));
        }
        assert_eq!(Diet::parse("carnivore"), None);
    }

    #[test]
    fn season_round_trip() {
        for season in Season::ALL {
            assert_eq!(Season::parse(season.as_str()), Some(season));
        }
        assert_eq!(Season::parse(""), None);
    }

    #[test]
    fn every_user_has_role_user() {
        let user = User {
            id: 1,
            username: "admin".into(),
            roles: vec!["ROLE_ADMIN".into()],
            password: String::new(),
        };
        assert!(user.has_role("ROLE_USER"));
        assert!(user.has_role("ROLE_ADMIN"));
        assert!(!user.has_role("ROLE_OTHER"));
    }
}
