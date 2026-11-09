//! Default layout of a new vault: folders, templates and welcome note.
//! Applied only to an empty folder: an existing vault is never modified.

use crate::lang::Lang;
use crate::settings::Settings;
use crate::vault::Vault;
use crate::Result;

/// The folder contains nothing (apart from the internal `.eloria` folder).
pub fn is_blank(vault: &Vault) -> Result<bool> {
    for e in std::fs::read_dir(vault.root())? {
        let name = e?.file_name().to_string_lossy().to_string();
        if name != crate::ELORIA_DIR {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Creates the default layout and returns the created paths (folders and notes).
pub fn scaffold(vault: &Vault, s: &Settings) -> Result<Vec<String>> {
    let lang = s.lang();
    let concepts = lang.pick("Concepts", "Concepts");
    let these = lang.pick("Thèse", "Thesis");
    let mut created = Vec::new();
    for dir in [s.sources_folder.as_str(), s.fiches_folder.as_str(), concepts, these, s.daily_folder.as_str(), s.templates_folder.as_str(), s.attachments_folder.as_str()] {
        let dir = dir.trim_matches('/');
        if !dir.is_empty() && !vault.exists(dir) {
            vault.create_folder(dir)?;
            created.push(dir.to_string());
        }
    }
    let tpl = s.templates_folder.trim_matches('/');
    let mut files: Vec<(String, String)> = vec![
        (format!("{tpl}/Journal.md"), journal(lang)),
        (format!("{tpl}/{}.md", lang.pick("Note de concept", "Concept note")), concept(lang)),
        (format!("{tpl}/{}.md", lang.pick("Réunion", "Meeting")), meeting(lang)),
        (format!("{these}/{}.md", lang.pick("Plan de thèse", "Thesis outline")), outline(lang)),
    ];
    files.push((lang.pick("Bienvenue.md", "Welcome.md").to_string(), welcome(lang, s, concepts, these)));
    for (rel, content) in files {
        if !vault.exists(&rel) {
            vault.create_note(&rel, &content)?;
            created.push(rel);
        }
    }
    Ok(created)
}

fn journal(l: Lang) -> String {
    match l {
        Lang::Fr => "# {{title}}\n\n## Objectifs du jour\n- [ ] \n\n## Lectures\n\n\n## Idées\n\n\n## Bilan\n\n".into(),
        Lang::En => "# {{title}}\n\n## Today's goals\n- [ ] \n\n## Reading\n\n\n## Ideas\n\n\n## Review\n\n".into(),
    }
}

fn concept(l: Lang) -> String {
    match l {
        Lang::Fr => "---\ntype: concept\ntags: [concept]\nchapitres: []\n---\n# {{title}}\n\n> [!definition] Définition\n> \n\n## Explication\n\n\n## Sources\n- [@]\n\n## Notes liées\n- [[ ]]\n".into(),
        Lang::En => "---\ntype: concept\ntags: [concept]\nchapitres: []\n---\n# {{title}}\n\n> [!definition] Definition\n> \n\n## Explanation\n\n\n## Sources\n- [@]\n\n## Related notes\n- [[ ]]\n".into(),
    }
}

fn meeting(l: Lang) -> String {
    match l {
        Lang::Fr => "---\ntype: réunion\ndate: {{date}}\n---\n# {{title}}\n\n**Participants :** \n\n## Ordre du jour\n\n\n## Notes\n\n\n## Décisions\n\n\n## Actions\n- [ ] \n".into(),
        Lang::En => "---\ntype: meeting\ndate: {{date}}\n---\n# {{title}}\n\n**Attendees:** \n\n## Agenda\n\n\n## Notes\n\n\n## Decisions\n\n\n## Action items\n- [ ] \n".into(),
    }
}

fn outline(l: Lang) -> String {
    match l {
        Lang::Fr => "---\ntype: plan\ntags: [thèse]\n---\n# Plan de thèse\n\n> [!astuce] Chapitres\n> Crée un chapitre depuis le panneau Thèse (bouton +) : il reçoit `type: chapitre`, un identifiant (`ch1`) et un objectif en mots. Rattache ensuite fiches et concepts avec `chapitres: [ch1]`.\n\n## Question de recherche\n\n\n## Chapitres\n1. \n".into(),
        Lang::En => "---\ntype: plan\ntags: [thesis]\n---\n# Thesis outline\n\n> [!tip] Chapters\n> Create a chapter from the Thesis panel (+ button): it gets `type: chapitre`, an identifier (`ch1`) and a word target. Then link reading notes and concepts with `chapitres: [ch1]`.\n\n## Research question\n\n\n## Chapters\n1. \n".into(),
    }
}

fn welcome(l: Lang, s: &Settings, concepts: &str, these: &str) -> String {
    let (sources, fiches, journal, tpl, att) = (&s.sources_folder, &s.fiches_folder, &s.daily_folder, &s.templates_folder, &s.attachments_folder);
    match l {
        Lang::Fr => format!(
            "# Bienvenue dans ton coffre\n\nCe dossier est prêt à l'emploi :\n\n\
- **{sources}** : les documents importés de Zotero, collections reprises telles quelles.\n\
- **{fiches}** : tes fiches de lecture, jointes à une source (« Créer une fiche »).\n\
- **{concepts}** : une note par concept, reliée à tes lectures avec `[[liens]]`.\n\
- **{these}** : ton plan et tes chapitres ; le panneau Thèse suit l'avancement.\n\
- **{journal}** : la note du jour (`Ctrl+Maj+D`).\n\
- **{tpl}** : les modèles (`Ctrl+Maj+N`).\n\
- **{att}** : les images collées dans les notes.\n\n\
> [!astuce] Pour commencer\n\
> 1. Ouvre le panneau Zotero et synchronise ta bibliothèque.\n\
> 2. Crée une fiche de lecture, ou importe toute la bibliothèque en notes.\n\
> 3. Tape `/` dans une note pour insérer un bloc, `[[` pour un lien, `[@` pour citer.\n"
        ),
        Lang::En => format!(
            "# Welcome to your vault\n\nThis folder is ready to use:\n\n\
- **{sources}**: documents imported from Zotero, collections kept as they are.\n\
- **{fiches}**: your reading notes, attached to a source (\"Create a note\").\n\
- **{concepts}**: one note per concept, linked to your readings with `[[links]]`.\n\
- **{these}**: your outline and chapters; the Thesis panel tracks progress.\n\
- **{journal}**: the daily note (`Ctrl+Shift+D`).\n\
- **{tpl}**: templates (`Ctrl+Shift+N`).\n\
- **{att}**: images pasted into notes.\n\n\
> [!tip] Getting started\n\
> 1. Open the Zotero panel and sync your library.\n\
> 2. Create a reading note, or import the whole library as notes.\n\
> 3. Type `/` in a note to insert a block, `[[` for a link, `[@` to cite.\n"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_vault_only() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        assert!(is_blank(&v).unwrap());
        let made = scaffold(&v, &Settings::default()).unwrap();
        for p in ["Sources", "Fiches de lecture", "Concepts", "Thèse", "Journal", "Modèles", "Pièces jointes", "Modèles/Journal.md", "Bienvenue.md", "Thèse/Plan de thèse.md"] {
            assert!(v.exists(p), "{p} manquant");
        }
        assert!(made.len() >= 10);
        assert!(!is_blank(&v).unwrap());
        // A second run creates nothing.
        assert!(scaffold(&v, &Settings::default()).unwrap().is_empty());

        let d2 = tempfile::tempdir().unwrap();
        let v2 = Vault::open(d2.path()).unwrap();
        let en = Settings { language: "en".into(), ..Default::default() };
        scaffold(&v2, &en).unwrap();
        assert!(v2.exists("Welcome.md") && v2.exists("Thesis/Thesis outline.md") && v2.exists("Modèles/Meeting.md"));
    }
}
