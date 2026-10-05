use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};

pub struct TokenStore {
    path: PathBuf,
}

impl TokenStore {
    pub fn new() -> Result<Self> {
        let path = dirs::config_local_dir()
            .context("Não foi possível encontrar LOCALAPPDATA")?
            .join("AgnesProvider")
            .join("tokens.txt");
        
        Ok(Self { path })
    }

    pub fn ensure_dir(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    pub fn load(&self) -> Result<Vec<String>> {
        self.ensure_dir()?;
        
        if !self.path.exists() {
            fs::write(&self.path, "")?;
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.path)?;
        let tokens: Vec<String> = content
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        Ok(tokens)
    }

    pub fn add(&self, token: &str) -> Result<()> {
        self.ensure_dir()?;
        
        let mut tokens = self.load()?;
        let token = token.trim().to_string();
        
        if tokens.contains(&token) {
            println!("Token já cadastrado.");
            return Ok(());
        }

        tokens.push(token);
        let content = tokens.join("\n");
        fs::write(&self.path, content)?;
        
        println!("Token adicionado. Total: {}", tokens.len());
        Ok(())
    }

    pub fn list(&self) -> Result<()> {
        let tokens = self.load()?;
        
        if tokens.is_empty() {
            println!("Nenhum token cadastrado.");
            return Ok(());
        }

        for (i, token) in tokens.iter().enumerate() {
            let masked = if token.len() > 12 {
                format!("{}{}{}", 
                    &token[..8],
                    "*".repeat(token.len() - 12),
                    &token[token.len()-4..])
            } else {
                "*".repeat(token.len())
            };
            println!("[{}] {}", i + 1, masked);
        }
        
        Ok(())
    }

    pub fn save(&self, tokens: &[String]) -> Result<()> {
        self.ensure_dir()?;
        let content = tokens.join("\n");
        fs::write(&self.path, content)?;
        Ok(())
    }
}
