use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::Duration;

use fs2::FileExt;
use git2::{Oid, Repository, Signature, Status};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::models::DiffHunk;

use super::types::{RoleTemplate, TeamPreset};

mod git;
mod presets;
mod roles;
mod state;

#[cfg(test)]
mod tests;

const TEMPLATES_DIRNAME: &str = "templates";
const ROLES_DIRNAME: &str = "roles";
const PRESETS_DIRNAME: &str = "presets";
const META_DIRNAME: &str = "_meta";
const GITIGNORE_FILENAME: &str = ".gitignore";
const LOCK_FILENAME: &str = ".lock";
const LOCK_FALLBACK_FILENAME: &str = ".lock.fallback";
const STATE_FILENAME: &str = "state.json";
const RECOVERY_COMMIT_MESSAGE: &str = "templates: recovery auto-commit";
const DEFAULT_DEBOUNCE_WINDOW_SECS: i64 = 30;
const FALLBACK_LOCK_RETRY_DELAY_MS: u64 = 20;
const FALLBACK_LOCK_RETRY_ATTEMPTS: usize = 250;
const TEMP_FILE_RANDOM_RETRY_ATTEMPTS: usize = 16;
const BUILTIN_CATALOG_REVISION: u32 = 1;

const GITIGNORE_CONTENTS: &str = "_meta/state.json\n*.tmp*\n.lock\n.lock.fallback\n";

// Exact SHA-256 fingerprints of template bytes shipped from 0.4.5 (the first
// bundled catalog) through 0.8.5. A file must match both an old path and known
// shipped bytes before the catalog migration may remove it; locally edited
// copies remain user-owned.
//
// Forward contract: `seed_builtins_if_missing` re-copies the current bundle
// into the user store on every mutation path, so reconciliation alone does NOT
// keep already-migrated stores clean. Every future bundled-template edit must
// bump `BUILTIN_CATALOG_REVISION` and append the superseded bytes'
// fingerprints here.
const PREVIOUS_BUNDLED_TEMPLATE_HASHES: &[(&str, &str)] = &[
    (
        "presets/dev-team.yaml",
        "0b21738499d30483be03427845cca63da1bd399caf4431d634790876749f28ed",
    ),
    (
        "presets/dev-team.yaml",
        "bf753d6513394eb6c61e33788b1b65f77e2d3e7fd05b4f4bbf559ec257980648",
    ),
    (
        "presets/full-team.yaml",
        "d39d3082c563769a249246b769dd2f46c612eaea0f7877c1d122aafba67c44a0",
    ),
    (
        "presets/full-team.yaml",
        "1352a9cb3b709188d1f8bfd52ae69de7c90cae164d5e32bf0b87dc92a9ea720f",
    ),
    (
        "presets/grok-pair.yaml",
        "869793213f7aeb8c204719ff1dc726c9b0211f1b0908b3d8c907996685c14c72",
    ),
    (
        "presets/grok-pair.yaml",
        "4044bfd247c210dd44506fac800af8d2de2b3c9b90394030b532297b3faa1803",
    ),
    (
        "presets/pair.yaml",
        "9f81fce260790bd02215c9b596fe32465e17b3872384cfb7df662fe0e8c9087f",
    ),
    (
        "presets/research-team.yaml",
        "16196e7a24218a3679fff2a6fcda41f6e4ca8bb195a34c02eb7c000c7b9813ee",
    ),
    (
        "presets/research-team.yaml",
        "06be090b1c326440526554415f9967c2adf0436c8fc578b0a364fb0157c82021",
    ),
    (
        "roles/adversarial-reviewer-claude.yaml",
        "88455a7e05876400d23f6bc10a6946bcfd14f297d8825aaaf16e058399a9e5e2",
    ),
    (
        "roles/antigravity-orchestrator.yaml",
        "4f8934db51767a484b397693cc39ebfa7b350dd382bc5e1c42b27aa3049b1c78",
    ),
    (
        "roles/antigravity-ui-specialist.yaml",
        "7c088eef6e08c3a2721fd096c575611b0c20b8fe63489129c5b80ee8032ca8c4",
    ),
    (
        "roles/claude-design-lead.yaml",
        "8a7fb6ada013ee8d287ab5fa1742be204d9776a424f4846a66511e93d0351b13",
    ),
    (
        "roles/claude-orchestrator.yaml",
        "1a78b6d3caa1b49d39ed30cce5b628cef3cac1d1cced327e4396c67a8b686dd6",
    ),
    (
        "roles/claude-product-checker.yaml",
        "8e928d24bf3bcd22ea21f8aec1e6bfd5c4aaeebc0cb382cc4c1e6794b961513c",
    ),
    (
        "roles/claude-researcher.yaml",
        "c02160b84df2c472d30d3d90fc63c733468f4b1dcedc55a60751e007cf13536a",
    ),
    (
        "roles/claude-reviewer.yaml",
        "0e0e0fd25390d779003af92edaf102bca08f4b49019c0c3d70207698198361dd",
    ),
    (
        "roles/codex-architect.yaml",
        "b258f6df20bbbc58d3e80e096c62125486817771a904fd5f87f09e86e15e62e4",
    ),
    (
        "roles/codex-developer.yaml",
        "8e4ef423a83fb05345e0ed85de67f72071fecb6c46adf6dce17ecb6e9ace0865",
    ),
    (
        "roles/codex-orchestrator.yaml",
        "56fa5a6c81e8f60cbacea56e6b3d20e947188f6533ccb4e5aae0fbc17c97e213",
    ),
    (
        "roles/codex-product-lead.yaml",
        "a2aadfd219c4cd0f7ca127c90e1a6a458f7c52329300bd01efaa8ee0a691bc77",
    ),
    (
        "roles/codex-qa.yaml",
        "5e1bed1a9ab767147411a79da1bd4f4f66a5604a0626fc5e252ec428a6d7c8ce",
    ),
    (
        "roles/codex-vertical-slice-developer.yaml",
        "3fc7b2da5f99b8d1063ec654f6329f572e6792b206623b86ddf1370de16369b9",
    ),
    (
        "roles/docs-verifier-codex.yaml",
        "f99584b53393743b7b45915775a9c9bdd2d70e4f544265fe0882407badee31b5",
    ),
    (
        "roles/frontend-design-skill-developer.yaml",
        "4f00b61eb26e3427713be0256d298333771235e570673c72d6d91f1af02bf251",
    ),
    (
        "roles/grok-developer.yaml",
        "f320dd351e40ce9ad60df393e758c4c894b51c86eaa8c5f8ee6d47ceaae78dc2",
    ),
    (
        "roles/quick-dev-codex.yaml",
        "0bd3723a6371fa7ee26494f24538482df5a1c4dfa26e2bf7b69cedcd48fe9058",
    ),
    (
        "roles/taurhaus-architect.yaml",
        "da6f7d5925563b8c293e3101b458149d36339faebb2ebdbd00ed4946bc8a96be",
    ),
    (
        "roles/taurhaus-designer.yaml",
        "83074a1b8331f963bf3e6dd1adda751f7a83f4f177087525840a703479887b89",
    ),
    (
        "roles/taurhaus-developer.yaml",
        "b6479a34469f6841564fd63ea4a7f8b2ee276689dc3765c6173366009e2f5cd6",
    ),
    (
        "roles/taurhaus-lead-claude.yaml",
        "8fb95527acd038562f821f60a2f24492af0685cf5adc6ff0da74c4af506e79be",
    ),
    (
        "roles/taurhaus-lead-codex.yaml",
        "15732abf22322f2b760fa59f4df2615a953644cf52b3eefd7e170ad36cca348b",
    ),
    (
        "roles/v2-architect-claude.yaml",
        "2249efc466d309cadece5506fcb741846f7b022da5167398085f67893bda34e0",
    ),
    (
        "roles/v2-architect-codex.yaml",
        "4deb2843884cfd9aace251ef2b18cb97cc48dcc803bf800e2d51426768e2dfa5",
    ),
    (
        "roles/v2-design-lead-claude.yaml",
        "e783c0e24f043b060c1cf5d54ccad6b3c23496e653f64fae2c8bcf009e1bf16c",
    ),
    (
        "roles/v2-developer-claude.yaml",
        "24378b7f1ffb3ce1c7a48d977c8216c8c99790c036759785cca2e30fd0f69e3c",
    ),
    (
        "roles/v2-developer-codex.yaml",
        "c33c164de1f92f769885f45dbb1adc9b3a8c1599088b85505930e1e403b26f61",
    ),
    (
        "roles/v2-lead-claude.yaml",
        "ea9519cef9ca21d82ca84ce709b5f2432878b955a48d73bb4f5a964acac2480c",
    ),
    (
        "roles/v2-lead-codex.yaml",
        "bf07a10022a86b39ef7530d87fb0351bec0eecc136d5272561b17b25dbd2e00b",
    ),
    (
        "roles/v2-product-checker-claude.yaml",
        "9af635391efdb3207237a77454ed539651eea0646af074470da232778264dda8",
    ),
    (
        "roles/v3-architect-claude.yaml",
        "e24ce908c7f24be6cc891cf6f9bb5f770a42d6a5b3f8ea3dee9cf711fa9b08c2",
    ),
    (
        "roles/v3-architect-codex.yaml",
        "bcbaf9a5f2ba18bf6bb134cbca804e9f23854719b28f65f0460823e7b2b8cdf5",
    ),
    (
        "roles/v3-design-lead-claude.yaml",
        "7e9de33cf464a4344f4d4eb5498a600df22f176d82e681cff2e48d8988c781bd",
    ),
    (
        "roles/v3-developer-agy.yaml",
        "734fe7efacdcb9597e32db4d1c14f83dbd04b16c8a4926cf59e5f6598f605c3c",
    ),
    (
        "roles/v3-developer-claude.yaml",
        "82d543bb5f754a4f752a91c898335a58cab10cdf39fc92975469d52a69575144",
    ),
    (
        "roles/v3-developer-codex.yaml",
        "f815b1b4fb1d231b4ef758307d53d57efe52a05ffc54a1cc9873e9598a67341e",
    ),
    (
        "roles/v3-lead-claude.yaml",
        "9743b89d88ea1f706119cf2b5b2079348c56e3b14998e4635df9d31c63cd1a50",
    ),
    (
        "roles/v3-lead-claude.yaml",
        "48ad6b77969c9e37deaa5fc466c4e644315d71a1f1c8d46536deb46193f5c014",
    ),
    (
        "roles/v3-lead-codex.yaml",
        "353777544aff901fed99dc882fb6ddafce23f3e861a4fdbde49ce4092623c8e1",
    ),
    (
        "roles/v3-lead-codex.yaml",
        "db8a1f434df71e4fcf145fbebd1aaf5722dbe5a01485829ecba14053fc0390ac",
    ),
    (
        "roles/v3-product-checker-claude.yaml",
        "2240e616bafa22fc213c764fe5ecfb4efac4947d690f8c01b78d2040dceffcae",
    ),
    (
        "roles/v4-developer-agy.yaml",
        "1d006f6ddd06e895fe069814889ce8eb79778b2bdd85aa380923e071f6508dc3",
    ),
    (
        "roles/v4-developer-claude.yaml",
        "9a9db98fa92396f70011b3ef83f23e2888b17bf1942ec20f76c62dd6e8e91c38",
    ),
    (
        "roles/v4-developer-codex.yaml",
        "4dd3867ed317a686fc3231d7c9d3aabffb59944707cd2aa852b5595d0ba81fb2",
    ),
    (
        "roles/v4-developer-grok.yaml",
        "98f59514df7ba5cb046a654a29463bb48b30e174f6aaaf7b23dfe368f209341e",
    ),
    // Shipped v0.4.5 through v0.7.0 (generated from the release tags; see
    // the regression `a_v0_7_0_seeded_retired_role_reconciles_away`):
    (
        "presets/dev-team.yaml",
        "2941f85c72056f5f5f5d80fe146b0be149c9f3525e025622aa6975fa636d4476",
    ),
    (
        "presets/full-team.yaml",
        "12474a60eee7980a989898b4d5e8841d994f3323324e7d97030b2a6a67330fa0",
    ),
    (
        "presets/fullstack-dev-codex.yaml",
        "9f2ca8c6c8be7e23eb528f05ba780990c1b54e2492306180ac8213354c1a6879",
    ),
    (
        "presets/fullstack-dev-gemini.yaml",
        "25ce614a2a3f1c1f357132f331711d3c21a4c71275d133faba4f95a906596500",
    ),
    (
        "presets/fullstack-dev.yaml",
        "3ee18a890464aa19d2a4a6f8eca6183df9557d4f6c81c96ef44353645d0a9e13",
    ),
    (
        "presets/pair.yaml",
        "c107de3630ccb3b6d037353e8c741e80e6e43895e6710ca28d0bb3697cb44533",
    ),
    (
        "presets/research-dev-codex.yaml",
        "5761429db7ffaf983d211338131e1c7e7b3cb469a67cc7563d5f2c76ed1b76a2",
    ),
    (
        "presets/research-dev-gemini.yaml",
        "4001647ef14c22f50993dffaf3ff5a1be05d26efbf5846d2462848ab04c9a2a2",
    ),
    (
        "presets/research-dev.yaml",
        "51d517d829310d044d1e63ac3e745e852cd29e3eb3f9b3e0edb7fa1295f9e41a",
    ),
    (
        "presets/research-team.yaml",
        "9a73aa522144e30403c7e675e0096ec370609161dcdbbc68664c844ddeff97be",
    ),
    (
        "presets/review-team-codex.yaml",
        "b303521d366651c146dda0f2a6961caf20870bbd539107a0abf54250d5257fba",
    ),
    (
        "presets/review-team-gemini.yaml",
        "b08130f0302ab0289bacb5156a03a1f51b9e640fddcd5b21ea17d86200a1f311",
    ),
    (
        "presets/review-team.yaml",
        "5f3011e39c37c9ff7b5f62a0dfa62753260b62d80beb36e06285b8e41a625ac6",
    ),
    (
        "presets/standard-team-codex.yaml",
        "6a9488690a8f4634d1ce78f60c30fcbb614433a75213d3e17c835ea0c525dc0e",
    ),
    (
        "presets/standard-team-gemini.yaml",
        "91f79bec15d1a1d71b7e2c045db46ef7dd2e0271ccacd084befb0d286036d633",
    ),
    (
        "presets/standard-team.yaml",
        "6b88b9409b54e6e3219930ea383003e0d00a02104595cd962800e1798a5ce0bc",
    ),
    (
        "presets/taurhaus-standard-codex.yaml",
        "d7ef30f87a40dac2d1506781c19cdaa2b689dd90af76eaff8a7a69628b067a5c",
    ),
    (
        "presets/taurhaus-standard.yaml",
        "cf31a28180202eab880532fe39d749c2d3044cb4f36ac73b8ca4faffb838f9eb",
    ),
    (
        "roles/adversarial-reviewer-claude.yaml",
        "a23f6b988578c13d6718eb25068f621d0f462ac5c76fab296e355e44f1de5380",
    ),
    (
        "roles/claude-design-lead.yaml",
        "0bb004c7799e23659f2de40939b41b9541496331222e7159a45f56e855902e10",
    ),
    (
        "roles/claude-design-lead.yaml",
        "f576a347dbded3bf5b67e22c2f6393d12e2f0dcc7e74555f233c15305528bdb5",
    ),
    (
        "roles/claude-orchestrator.yaml",
        "09c8ae1f76b92d583f820d7997e0deb978e91d63da23ebb7daafe82c35c2dad5",
    ),
    (
        "roles/claude-orchestrator.yaml",
        "3781aacc3955aa00160f37ecbb067498dd27cecfc41faded78b6f5063be0392e",
    ),
    (
        "roles/claude-orchestrator.yaml",
        "bc3656baf9bcecebd9d6e07ce708b0d8da0027dd48e5f578b9d7f270fa325836",
    ),
    (
        "roles/claude-orchestrator.yaml",
        "f4c28cbec68913c9686db1976a3f13710b303d4ea1c554c5e8b46b3115a11559",
    ),
    (
        "roles/claude-product-checker.yaml",
        "1dc77cad85211531e22f9a2be9a1dab8e606cbf1844828505407bd290cf12577",
    ),
    (
        "roles/claude-product-checker.yaml",
        "fdff8a274b7829b627ccf954a3c73bd34015aea3391deb7493de1627d1408c10",
    ),
    (
        "roles/claude-researcher.yaml",
        "3c94718035550cebb09d7e4f7f14544779f151165ff56edbd42563ff4b964218",
    ),
    (
        "roles/claude-researcher.yaml",
        "4a618327083bc447adba57793a8ece05870127b8e78c440a4c277328ea4df4cf",
    ),
    (
        "roles/claude-researcher.yaml",
        "edd229b59902a330a1f081f8a694e3e5d7ee5c8e419c82eebe4b5e0348e4cbe4",
    ),
    (
        "roles/claude-researcher.yaml",
        "f5aa0fd280c4324a2cb8ed3f36926ef7911d0e6d1675b0387488dab7f798499b",
    ),
    (
        "roles/claude-reviewer.yaml",
        "2e7696c3928f79b67f49c73bfa38bd4106f88e6b503bb07545dbb10a08fcce1e",
    ),
    (
        "roles/claude-reviewer.yaml",
        "9332a173e3e219b34cf11ff3b4e6af0a6bf4519abbd3ecedf1f2e2630aaca3df",
    ),
    (
        "roles/claude-reviewer.yaml",
        "9340f7aed090a053696bebf863114e317230b36afc79ab44dd8400010ef92c58",
    ),
    (
        "roles/claude-reviewer.yaml",
        "a24c32553fbced187341ea9fc3c23d9e38f6d9df228de3ace564ee30ee7a4b6d",
    ),
    (
        "roles/codex-architect.yaml",
        "0e01b460f71cf680dce3122cc8819e603eff421957ad305d1519891033b151ab",
    ),
    (
        "roles/codex-architect.yaml",
        "122156fe6e1e40e89cde6da2a62274ae09198ba85559a142932282b6d815dc93",
    ),
    (
        "roles/codex-architect.yaml",
        "2c7c855d4b6c53432eeb8f1d2af3a0fb0b394072b348cf8b6b40e0f36c7ae2df",
    ),
    (
        "roles/codex-architect.yaml",
        "355687a935faeda6598f22eaa14d6bdd066f00dc3f40e5767f42207f574737a1",
    ),
    (
        "roles/codex-architect.yaml",
        "b8b85b0fa1d55f20245614c64696b34b8b5acd0cc26edd148e07790cb64adfa8",
    ),
    (
        "roles/codex-architect.yaml",
        "f5f4bba96467046d361ed17161b59f56b8cbbc59a9cd2360bea205759b81690d",
    ),
    (
        "roles/codex-developer.yaml",
        "11a863fa25af10439473fbca11751c8561076d327995aa86237c5faede657c92",
    ),
    (
        "roles/codex-developer.yaml",
        "3ed6d20fa996a8280f342b1c7e060864c93cde9f1aeb6a9e057a9d3d8e85d473",
    ),
    (
        "roles/codex-developer.yaml",
        "523d82e745d08b9a6230e1da42a291adeb5a5f9da88837f13ed6071929dc8010",
    ),
    (
        "roles/codex-developer.yaml",
        "8496d06ef7a60878a8684dbc0c982263faccc491b7dc1f995ac3820737ac86c9",
    ),
    (
        "roles/codex-developer.yaml",
        "b61070dfe9a26d84452431e754f2a86a1824bbad842f4d1a0e588de631eedfec",
    ),
    (
        "roles/codex-developer.yaml",
        "c92bdfbcf9f5f68b24bce337d69c99d93756591d8db51cb4c89a0f49af7c2354",
    ),
    (
        "roles/codex-developer.yaml",
        "ef58263711736c69a3535401d1508e7bac4f6297c57bb942e3b1e5deb92e2042",
    ),
    (
        "roles/codex-orchestrator.yaml",
        "c392b58eae6ba2732193886d87e9b559d26e52b530a99d2c4d9b930f2bc425e6",
    ),
    (
        "roles/codex-orchestrator.yaml",
        "f145a425461fc4b40dc33bfba2a98ea02c156ba2ae043fb5ba4c7fcc380fd7f1",
    ),
    (
        "roles/codex-product-lead.yaml",
        "9a130a48939bbf1ee07c6e9034015ede5e39a4609128831f497f7cebfd087eab",
    ),
    (
        "roles/codex-product-lead.yaml",
        "f84eb9f6816f733c2ef9f0301d45a4a6ab33518c370f1923d316063de14a4d5f",
    ),
    (
        "roles/codex-qa.yaml",
        "8ab170cd5b2b27eb15e30d43e66a577549c514b59bd88ab718cee410781b9359",
    ),
    (
        "roles/codex-qa.yaml",
        "8e88857f2a1e9484039cbf801203295623ca0a0e17bb09f98509d93dc353bb12",
    ),
    (
        "roles/codex-qa.yaml",
        "9cbcd962f7ad01cf76825716d576a29b7edf651902be9fd02a48a195373fea76",
    ),
    (
        "roles/codex-qa.yaml",
        "b15744a62eb0f5d0c7dc38e3b514ae1be0e70d53ec8ada883f63e0b1168d5483",
    ),
    (
        "roles/codex-qa.yaml",
        "b680825aff1b4f4bdd08468a68f4650a1167876a3685e73f699c0c6913b99400",
    ),
    (
        "roles/codex-qa.yaml",
        "d18873518022555e3a64131eb617d410084f7645de8ce341f3477cf535002c10",
    ),
    (
        "roles/codex-vertical-slice-developer.yaml",
        "d3cbc928315b5f9342ca86861bc5a068b43a3a6327789ac9f40186a5a73f586f",
    ),
    (
        "roles/codex-vertical-slice-developer.yaml",
        "e73c603b0a3dbf1d2a0ec26e9524e69a0881ee806943cbf22f1a34bb31a4ea82",
    ),
    (
        "roles/docs-verifier-codex.yaml",
        "943f4115ccf5a6e07ce97381996bdf6424f301af14571303369ed759b6b1339d",
    ),
    (
        "roles/frontend-design-skill-developer.yaml",
        "cb727662e580605f936362cfc06b1ef8e3152212c4839469f5d497195a295702",
    ),
    (
        "roles/gemini-orchestrator.yaml",
        "034f746595afab626143d921aeef15577bebf7d30b0e8302edc573c5134dc8f1",
    ),
    (
        "roles/gemini-orchestrator.yaml",
        "46735e1eeefe17acb5212730f6217b2a96cfc31182cc0ff415bfa0b5cfa8a452",
    ),
    (
        "roles/gemini-orchestrator.yaml",
        "8aff0ed25461b2b5caa7dcc1e298fd4605d72bd10ae4ae0eed8e27cd2968e88c",
    ),
    (
        "roles/gemini-ui-specialist.yaml",
        "0d1eb86d9dd2e7b3ec3082aab669cf9aa82b798e70597e378a45bc392bb3acc3",
    ),
    (
        "roles/gemini-ui-specialist.yaml",
        "16b85e149b00bf5e8e63e41b3c72c84f73ead992f2b3de5b2f58a0e297ac3a5a",
    ),
    (
        "roles/gemini-ui-specialist.yaml",
        "27fd5ded32647799b6c2cdd7fa7b3c8701154bb8cca51543b1b0423ab6478db2",
    ),
    (
        "roles/gemini-ui-specialist.yaml",
        "5e135878ddcdb7767bd1fed61bffae459ce7c1136a36a29a6eb10962aa903627",
    ),
    (
        "roles/gemini-ui-specialist.yaml",
        "c6f5ef79b703e33c2cbdd0bd7069600d8297e1242a39dbca0ef31c9d1b2ac551",
    ),
    (
        "roles/quick-dev-codex.yaml",
        "840a3213e0bc9af1ebbc4fd44bd1bff392a33e700cb48d794f39597ace12aaed",
    ),
    (
        "roles/taurhaus-architect.yaml",
        "623389a1620eedb4f038bb09001abcd685eb31f95048eb018ceec60bdfc06b12",
    ),
    (
        "roles/taurhaus-architect.yaml",
        "85790295b1f67cfb7b75c23f1b235ba552a96b40d78a9d5ac7c7ce43857cca99",
    ),
    (
        "roles/taurhaus-architect.yaml",
        "de178d2c0475911b835808ef620f0c29888c74bbafe50806b6f99491b70f8fa0",
    ),
    (
        "roles/taurhaus-designer.yaml",
        "13d9ab9dd7ef6291c060b3167c8e2caff84b44f3d4b0b5a7b600607c86b7a155",
    ),
    (
        "roles/taurhaus-designer.yaml",
        "27fc4ca2ac9230f6acfde873edaff564d2ea2005ca640b700a06afddf18fdc8e",
    ),
    (
        "roles/taurhaus-designer.yaml",
        "a4a52ef3cf685b13baf057405c7432a5aed4c0b694a6817a304154f5c40c4b9d",
    ),
    (
        "roles/taurhaus-designer.yaml",
        "fe4e3237a629cef78a0e855688a5cded77120f01f178918768eade90b65650ee",
    ),
    (
        "roles/taurhaus-developer.yaml",
        "01d1654ff16b37fd1f21a915b286411b923352a37bba2c397514c96cf238d639",
    ),
    (
        "roles/taurhaus-developer.yaml",
        "8b47a6f8b0e92db7b237ef33e8ad6963f34842303f445a26bcd9fb6e3ad53e15",
    ),
    (
        "roles/taurhaus-developer.yaml",
        "8dce6e828cf41e75b000b2fb5a92b011f91374bb3745c6a64f4993965404e94d",
    ),
    (
        "roles/taurhaus-lead-claude.yaml",
        "4cdcab554f8cd587422055b7430737eda181da9d2dd6c7f0f96f260f1e6d0392",
    ),
    (
        "roles/taurhaus-lead-claude.yaml",
        "7c26d4dd3303bc763232a0d8d72c61602f4cb4754bd7121c350eaa1b387115f3",
    ),
    (
        "roles/taurhaus-lead-claude.yaml",
        "fd9b5e7dafe0017689bf688154752a6c28512cf737ee58a5c99267f68b0a5717",
    ),
    (
        "roles/taurhaus-lead-codex.yaml",
        "27c231141c894439a494d8afaf6e9fdd183b224f10ccc1b587b914aef8ca2cd9",
    ),
    (
        "roles/taurhaus-lead-codex.yaml",
        "2c5c4145d7ff8821d58c5cbdec8dec4ef89e4abca6a250283cee2ffddacb182a",
    ),
    (
        "roles/taurhaus-lead-codex.yaml",
        "8af1c92231ee798997155ffedfb03ea4badace035029b450516b9e54bd9b8984",
    ),
    (
        "roles/v2-architect-claude.yaml",
        "0ce564039425da0cb01626e67cf446823c31fa6bfc8242963c2e9c3893e3f923",
    ),
    (
        "roles/v2-architect-claude.yaml",
        "d60720fddc457d511675acbc34aaaaa35d1fff365ce9fe9ce91442a87818db9c",
    ),
    (
        "roles/v2-architect-codex.yaml",
        "44c9401cd6f8fd1f55c4d882780be913b31f334a063dd552fb4694aba309d6be",
    ),
    (
        "roles/v2-architect-codex.yaml",
        "74959aef3b368a2aa86234a9b5a3905dc856f191bea03688074ac89d99807ae1",
    ),
    (
        "roles/v2-design-lead-claude.yaml",
        "755b0ac4b0fe183ede874a20f445b235b9c0dd172a25d347f45fd24401f0002b",
    ),
    (
        "roles/v2-design-lead-claude.yaml",
        "ff0906cbe00096caedbbb239c1c9720e7e230e07b5037e0c44ae676944f9821d",
    ),
    (
        "roles/v2-developer-claude.yaml",
        "5bfa27edefcfd185f4287f50bd70713dc6796e2d8214e26043bbd03f45317046",
    ),
    (
        "roles/v2-developer-claude.yaml",
        "7a54f5092199bc8ae3d60f0197e1dc9a96af0302f11df6ebebae4c61a432621d",
    ),
    (
        "roles/v2-developer-codex.yaml",
        "11e29e4fcc11335a2f52992627d3ab6d1b56dbe3251727d403fb29a6841350f1",
    ),
    (
        "roles/v2-developer-codex.yaml",
        "6ffdae0768cc289e95f016a5cb6d0a69615142b78af7b4d5ad07d55f0c680001",
    ),
    (
        "roles/v2-lead-claude.yaml",
        "352ffced42bc612fe86fc6960cbfdba94acecf175bb5271f0f0d4d0603437173",
    ),
    (
        "roles/v2-lead-claude.yaml",
        "c4386b589d2c9ee5fa64b8b1e4021bfc2850b18042b5459e30b358c5808b6ad4",
    ),
    (
        "roles/v2-lead-codex.yaml",
        "21b8be78958fd5c361827919a6449b663b72e878cbb8e64309b11c50517c30e6",
    ),
    (
        "roles/v2-lead-codex.yaml",
        "ab2705d9c09050a44fa0dd02874cc9cedfcb036fb418614194143a4fdccbfdb8",
    ),
    (
        "roles/v2-product-checker-claude.yaml",
        "ca95d20d04fe787670725ec241047c8e7088598468799a277575a5a63afa8801",
    ),
    (
        "roles/v2-product-checker-claude.yaml",
        "ece49b53048c6d610e5e9a64bd1eaf2b424da4c01f6370988cdb8f28957e3a13",
    ),
    (
        "roles/v3-architect-claude.yaml",
        "45a0b5361c28ce5c5575d4669ad49a59886a9ad0a6e3ae01ba342d195b7ed569",
    ),
    (
        "roles/v3-architect-claude.yaml",
        "bb6b033e542cb5cc7acc219ed33a9dc4f9ae2734d1aace45f58c2198ba7042df",
    ),
    (
        "roles/v3-architect-codex.yaml",
        "576d44ac05ebab44607a71cc915c2123471f74103f4f4b353d6c960f44e91144",
    ),
    (
        "roles/v3-architect-codex.yaml",
        "ef6b79193ea2a2d51de4a5a55107cf829374827af85ca72a7a22ae3a3d4a490d",
    ),
    (
        "roles/v3-design-lead-claude.yaml",
        "1b5dbd05b90f75db48fb90937c6e4686aa0c0a2cd414c4d838b594534fc12efa",
    ),
    (
        "roles/v3-design-lead-claude.yaml",
        "917c83538e13ded5a54add86ab3cffa8c6a734abee534a34eed18cfc2fc366e2",
    ),
    (
        "roles/v3-developer-claude.yaml",
        "0c0a1f21ebd76615e8df8d263e641a6813c697db2ad78174833380984eaca3e0",
    ),
    (
        "roles/v3-developer-claude.yaml",
        "fdd753046582f5261ff3aee20cf2eb6b8cbc5aa5b15aa79890dc641919ff84f8",
    ),
    (
        "roles/v3-developer-codex.yaml",
        "b83296bf6cbafe8356b53d21e08dff4b2c85ef51b8f1102e37c2e80d2c9dca11",
    ),
    (
        "roles/v3-developer-codex.yaml",
        "ff0bfd03a34041f7f980a956695693244e5752694cbaf29c6e4ddecab2b2df6d",
    ),
    (
        "roles/v3-lead-claude.yaml",
        "0fd84e2124cd354d9069ba8276cafda3e730de014b9e2be739683f7c3dfbbb5e",
    ),
    (
        "roles/v3-lead-claude.yaml",
        "4eaab814e0a3a2dd9fa81ebe7c420242ee15512603a6f0031b88d58ed21bb9b5",
    ),
    (
        "roles/v3-lead-codex.yaml",
        "24e44d261fe3234e3b932cbfb3e2a43da93aae0ebaabb43161bc5c830a129be1",
    ),
    (
        "roles/v3-lead-codex.yaml",
        "983e0d3518c0d13c549aa57de1c8c139b0afa73f510490a8f5f64b11512ade6f",
    ),
    (
        "roles/v3-product-checker-claude.yaml",
        "39543544370804a7b762216b255645610e7fa7533529d1c9739be393aa73d58e",
    ),
    (
        "roles/v3-product-checker-claude.yaml",
        "6de3b04cf0e22b37a7249e18deb06c238d1dcc98874278feeb457cf1dd04b9b9",
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateCatalog {
    pub roles: Vec<RoleTemplate>,
    pub presets: Vec<TeamPreset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSource {
    BuiltIn,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleTemplateRecord {
    pub template: RoleTemplate,
    pub source: TemplateSource,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamPresetRecord {
    pub template: TeamPreset,
    pub source: TemplateSource,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCommit {
    pub commit_id: String,
    pub short_id: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
    pub changed_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCommitPage {
    pub commits: Vec<TemplateCommit>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDiffFile {
    pub path: String,
    pub status: String,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDiffStats {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDiff {
    pub commit_id: String,
    pub files: Vec<TemplateDiffFile>,
    pub stats: TemplateDiffStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateMutationResult {
    pub commit_id: Option<String>,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateFileMutation {
    pub relative_path: PathBuf,
    pub contents: Option<Vec<u8>>,
}

impl TemplateFileMutation {
    pub fn write(relative_path: PathBuf, contents: Vec<u8>) -> Self {
        Self {
            relative_path,
            contents: Some(contents),
        }
    }

    pub fn delete(relative_path: PathBuf) -> Self {
        Self {
            relative_path,
            contents: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TemplateStoreState {
    #[serde(default)]
    pub pending_actions: Vec<PendingAction>,
    pub last_commit_at: Option<i64>,
    #[serde(default)]
    pub repo_initialized: bool,
    #[serde(default)]
    pub builtin_catalog_revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct DebounceState {
    pending_actions: Vec<PendingAction>,
    window_secs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingAction {
    pub action: String,
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub changed_paths: Vec<String>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum TemplateStoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid template path: {0}")]
    InvalidTemplatePath(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Read-only template: {0}")]
    ReadOnly(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Lock timeout: {0}")]
    LockTimeout(String),
}

#[derive(Debug, Clone)]
struct PathChange {
    path: PathBuf,
    deleted: bool,
}

#[derive(Debug, Clone)]
struct MutationDescriptor {
    action: String,
    kind: String,
    id: String,
}

#[derive(Debug)]
struct FallbackLockGuard {
    path: PathBuf,
    _file: File,
}

impl Drop for FallbackLockGuard {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            if err.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    lock_path = %self.path.display(),
                    error = %err,
                    "failed to remove template fallback lockfile"
                );
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct TemplateStoreLockGuard {
    _advisory_file: Option<File>,
    _fallback_guard: Option<FallbackLockGuard>,
}

impl TemplateStoreLockGuard {
    fn advisory(file: File) -> Self {
        Self {
            _advisory_file: Some(file),
            _fallback_guard: None,
        }
    }

    fn fallback(guard: FallbackLockGuard) -> Self {
        Self {
            _advisory_file: None,
            _fallback_guard: Some(guard),
        }
    }
}

#[derive(Debug, Clone)]
struct RoleTemplateFile {
    template: RoleTemplate,
}

#[derive(Debug, Clone)]
struct TeamPresetFile {
    template: TeamPreset,
}

/// How a directory scan treats a file it cannot parse. Every read path is
/// `SkipUnparseable` — one broken or stray file must not take listings or the
/// merged catalog down — while pre-commit validation is `Strict`: a corrupt
/// store must never be committed into template history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanMode {
    Strict,
    SkipUnparseable,
}

impl DebounceState {
    fn from_store(state: TemplateStoreState, window_secs: i64) -> Self {
        Self {
            pending_actions: state.pending_actions,
            window_secs,
        }
    }

    fn is_empty(&self) -> bool {
        self.pending_actions.is_empty()
    }

    fn oldest_first_seen(&self) -> Option<i64> {
        self.pending_actions.iter().map(|a| a.first_seen_at).min()
    }

    fn should_flush_lazy(&self, now_ts: i64) -> bool {
        self.oldest_first_seen()
            .map(|oldest| now_ts.saturating_sub(oldest) >= self.window_secs)
            .unwrap_or(false)
    }

    fn enqueue(&mut self, descriptor: MutationDescriptor, changed_paths: &[PathBuf], now_ts: i64) {
        if let Some(existing) = self
            .pending_actions
            .iter_mut()
            .find(|action| action.kind == descriptor.kind && action.id == descriptor.id)
        {
            existing.action = descriptor.action;
            existing.last_seen_at = now_ts;
            for path in changed_paths {
                let path_str = path.to_string_lossy().to_string();
                if !existing
                    .changed_paths
                    .iter()
                    .any(|existing| existing == &path_str)
                {
                    existing.changed_paths.push(path_str);
                }
            }
            return;
        }

        self.pending_actions.push(PendingAction {
            action: descriptor.action,
            kind: descriptor.kind,
            id: descriptor.id,
            changed_paths: changed_paths
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
            first_seen_at: now_ts,
            last_seen_at: now_ts,
        });
    }

    fn commit_message(&self) -> String {
        if self.pending_actions.len() == 1 {
            let action = &self.pending_actions[0];
            return format!("templates: {} {} {}", action.action, action.kind, action.id);
        }
        format!("templates: batch {} changes", self.pending_actions.len())
    }

    fn shutdown_message(&self) -> String {
        format!(
            "templates: shutdown flush {} changes",
            self.pending_actions.len()
        )
    }

    fn take_changed_paths(&self) -> Vec<PathBuf> {
        let mut unique = BTreeMap::<PathBuf, bool>::new();
        for action in &self.pending_actions {
            for raw in &action.changed_paths {
                let path = PathBuf::from(raw);
                unique.entry(path).or_insert(true);
            }
        }
        unique.into_keys().collect()
    }
}

#[derive(Debug, Clone)]
pub struct TemplateStore {
    templates_dir: PathBuf,
    builtins_dir: PathBuf,
    debounce_window_secs: i64,
}

impl TemplateStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            templates_dir: app_data_dir.join(TEMPLATES_DIRNAME),
            builtins_dir: default_builtins_dir(),
            debounce_window_secs: DEFAULT_DEBOUNCE_WINDOW_SECS,
        }
    }

    pub fn with_builtins_dir(app_data_dir: PathBuf, builtins_dir: PathBuf) -> Self {
        Self::with_builtins_and_debounce(app_data_dir, builtins_dir, DEFAULT_DEBOUNCE_WINDOW_SECS)
    }

    pub fn with_builtins_and_debounce(
        app_data_dir: PathBuf,
        builtins_dir: PathBuf,
        debounce_window_secs: i64,
    ) -> Self {
        Self {
            templates_dir: app_data_dir.join(TEMPLATES_DIRNAME),
            builtins_dir,
            debounce_window_secs,
        }
    }

    pub fn templates_dir(&self) -> &Path {
        &self.templates_dir
    }

    pub fn ensure_directories(&self) -> Result<(), TemplateStoreError> {
        fs::create_dir_all(self.roles_dir())?;
        fs::create_dir_all(self.presets_dir())?;
        fs::create_dir_all(self.meta_dir())?;
        Ok(())
    }

    pub fn load_catalog(&self) -> Result<TemplateCatalog, TemplateStoreError> {
        self.reconcile_builtins_if_needed()?;
        self.load_catalog_without_reconcile()
    }

    /// Pre-commit validation: every file in the store must parse — a stray
    /// or corrupt YAML that the tolerant read paths would skip is a reason
    /// NOT to commit — and the merged catalog must load.
    pub(super) fn validate_store_strict(&self) -> Result<(), TemplateStoreError> {
        for dir in [self.builtins_dir.join(ROLES_DIRNAME), self.roles_dir()] {
            self.load_role_files_from_dir_with(&dir, ScanMode::Strict)?;
        }
        for dir in [self.builtins_dir.join(PRESETS_DIRNAME), self.presets_dir()] {
            self.load_preset_files_from_dir_with(&dir, ScanMode::Strict)?;
        }
        self.load_catalog_without_reconcile()?;
        Ok(())
    }

    fn load_catalog_without_reconcile(&self) -> Result<TemplateCatalog, TemplateStoreError> {
        let roles = self.load_role_catalog()?;
        let presets = self.load_preset_catalog(&roles)?;

        Ok(TemplateCatalog { roles, presets })
    }

    fn reconcile_builtins_if_needed(&self) -> Result<(), TemplateStoreError> {
        self.ensure_directories()?;
        if self.load_state_unlocked()?.builtin_catalog_revision >= BUILTIN_CATALOG_REVISION {
            return Ok(());
        }

        let lock = self.acquire_lock()?;
        if self.load_state_unlocked()?.builtin_catalog_revision >= BUILTIN_CATALOG_REVISION {
            return Ok(());
        }

        let mutations = self.builtin_reconciliation_mutations()?;
        let changed_paths = mutations
            .iter()
            .map(|mutation| mutation.relative_path.clone())
            .collect::<Vec<_>>();

        for mutation in &mutations {
            let target = self.templates_dir.join(&mutation.relative_path);
            match mutation.contents.as_ref() {
                Some(contents) => write_atomic_file(&target, contents)?,
                None => match fs::remove_file(&target) {
                    Ok(()) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => return Err(TemplateStoreError::Io(err)),
                },
            }
        }

        if !changed_paths.is_empty() && self.git_dir().is_dir() {
            let repo = Repository::open(self.templates_dir())?;
            let changes = changed_paths
                .iter()
                .map(|path| PathChange {
                    path: path.clone(),
                    deleted: !self.templates_dir.join(path).exists(),
                })
                .collect::<Vec<_>>();
            let _ = self.commit_with_repo(
                &repo,
                &changes,
                &format!("templates: reconcile built-in catalog v{BUILTIN_CATALOG_REVISION}"),
            )?;
        }

        let mut state = self.load_state_unlocked()?;
        state.builtin_catalog_revision = BUILTIN_CATALOG_REVISION;
        self.save_state_unlocked(&state)?;
        drop(lock);
        Ok(())
    }

    fn builtin_reconciliation_mutations(
        &self,
    ) -> Result<Vec<TemplateFileMutation>, TemplateStoreError> {
        let mut mutations = Vec::new();
        let mut removed_preset_paths = BTreeSet::new();
        let mut preset_entries = fs::read_dir(self.presets_dir())?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        preset_entries.sort();

        for target in &preset_entries {
            if !target.is_file() || !is_yaml_file(target) {
                continue;
            }
            let Some(file_name) = target.file_name() else {
                continue;
            };
            let relative = PathBuf::from(PRESETS_DIRNAME).join(file_name);
            let existing = fs::read(target)?;
            if was_previously_shipped_builtin(&relative, &existing) {
                removed_preset_paths.insert(relative.clone());
                mutations.push(TemplateFileMutation::delete(relative));
            }
        }

        let mut referenced_role_ids = BTreeSet::new();
        for preset in self.load_presets_from_dir(&self.builtins_dir.join(PRESETS_DIRNAME))? {
            insert_preset_role_ids(&preset, &mut referenced_role_ids);
        }
        for target in preset_entries {
            if !target.is_file() || !is_yaml_file(&target) {
                continue;
            }
            let Some(file_name) = target.file_name() else {
                continue;
            };
            let relative = PathBuf::from(PRESETS_DIRNAME).join(file_name);
            if removed_preset_paths.contains(&relative) {
                continue;
            }
            let raw = fs::read_to_string(&target)?;
            // Reference-gathering must not take the whole catalog down: a
            // stray or unparseable YAML in the user presets directory names
            // no roles and is skipped with a warning, instead of failing
            // every role read behind the reconcile.
            match serde_norway::from_str::<TeamPreset>(&raw) {
                Ok(preset) => insert_preset_role_ids(&preset, &mut referenced_role_ids),
                Err(err) => {
                    tracing::warn!(
                        path = %target.display(),
                        error = %err,
                        "skipping unparseable user preset during builtin reconciliation"
                    );
                }
            }
        }

        let mut role_entries = fs::read_dir(self.roles_dir())?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        role_entries.sort();
        for target in role_entries {
            if !target.is_file() || !is_yaml_file(&target) {
                continue;
            }
            let Some(file_name) = target.file_name() else {
                continue;
            };
            let relative = PathBuf::from(ROLES_DIRNAME).join(file_name);
            let existing = fs::read(&target)?;
            if !was_previously_shipped_builtin(&relative, &existing) {
                continue;
            }

            let bundled = self.builtins_dir.join(&relative);
            if !bundled.is_file() {
                let raw = String::from_utf8(existing).map_err(|err| {
                    TemplateStoreError::Parse(format!(
                        "failed to read role {} as UTF-8: {err}",
                        target.display()
                    ))
                })?;
                let role = serde_norway::from_str::<RoleTemplate>(&raw).map_err(|err| {
                    TemplateStoreError::Parse(format!(
                        "failed to parse role {}: {err}",
                        target.display()
                    ))
                })?;
                if referenced_role_ids.contains(&role.role_id) {
                    continue;
                }
            }
            mutations.push(TemplateFileMutation::delete(relative));
        }
        Ok(mutations)
    }

    fn load_role_catalog(&self) -> Result<Vec<RoleTemplate>, TemplateStoreError> {
        self.ensure_directories()?;

        let mut roles_by_id: BTreeMap<String, RoleTemplate> = BTreeMap::new();
        for role in self.load_role_templates_from_dir(&self.builtins_dir.join(ROLES_DIRNAME))? {
            roles_by_id.insert(role.role_id.clone(), role);
        }
        for role in self.load_role_templates_from_dir(&self.roles_dir())? {
            roles_by_id.insert(role.role_id.clone(), role);
        }

        let roles = roles_by_id.into_values().collect::<Vec<_>>();
        for role in &roles {
            role.validate()
                .map_err(|err| TemplateStoreError::Parse(err.to_string()))?;
        }
        Ok(roles)
    }

    fn load_preset_catalog(
        &self,
        role_catalog: &[RoleTemplate],
    ) -> Result<Vec<TeamPreset>, TemplateStoreError> {
        self.ensure_directories()?;

        let mut presets_by_id: BTreeMap<String, TeamPreset> = BTreeMap::new();
        for preset in self.load_presets_from_dir(&self.builtins_dir.join(PRESETS_DIRNAME))? {
            if let Err(err) = preset.validate_with_role_catalog(role_catalog) {
                tracing::warn!(
                    preset_id = %preset.preset_id,
                    source = "built_in",
                    error = %err,
                    "skipping invalid team preset"
                );
                continue;
            }
            presets_by_id.insert(preset.preset_id.clone(), preset);
        }
        for preset in self.load_presets_from_dir(&self.presets_dir())? {
            if let Err(err) = preset.validate_with_role_catalog(role_catalog) {
                tracing::warn!(
                    preset_id = %preset.preset_id,
                    source = "user",
                    error = %err,
                    "skipping invalid team preset"
                );
                continue;
            }
            presets_by_id.insert(preset.preset_id.clone(), preset);
        }
        Ok(presets_by_id.into_values().collect())
    }

    fn roles_dir(&self) -> PathBuf {
        self.templates_dir.join(ROLES_DIRNAME)
    }

    fn presets_dir(&self) -> PathBuf {
        self.templates_dir.join(PRESETS_DIRNAME)
    }

    fn meta_dir(&self) -> PathBuf {
        self.templates_dir.join(META_DIRNAME)
    }

    fn git_dir(&self) -> PathBuf {
        self.templates_dir.join(".git")
    }

    fn state_path(&self) -> PathBuf {
        self.meta_dir().join(STATE_FILENAME)
    }

    fn gitignore_path(&self) -> PathBuf {
        self.templates_dir.join(GITIGNORE_FILENAME)
    }

    fn ensure_gitignore(&self) -> Result<(), TemplateStoreError> {
        let gitignore = self.gitignore_path();
        if gitignore.exists() {
            return Ok(());
        }
        write_atomic_file(&gitignore, GITIGNORE_CONTENTS.as_bytes())
    }

    fn seed_builtins_if_missing(&self) -> Result<(), TemplateStoreError> {
        self.copy_missing_from_dir(&self.builtins_dir.join(ROLES_DIRNAME), &self.roles_dir())?;
        self.copy_missing_from_dir(
            &self.builtins_dir.join(PRESETS_DIRNAME),
            &self.presets_dir(),
        )?;
        Ok(())
    }

    fn copy_missing_from_dir(
        &self,
        source_dir: &Path,
        target_dir: &Path,
    ) -> Result<(), TemplateStoreError> {
        if !source_dir.exists() {
            return Ok(());
        }

        fs::create_dir_all(target_dir)?;

        let mut entries = fs::read_dir(source_dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        entries.sort();

        for source_path in entries {
            if !source_path.is_file() {
                continue;
            }
            if !is_yaml_file(&source_path) {
                continue;
            }

            let Some(file_name) = source_path.file_name() else {
                continue;
            };
            let target_path = target_dir.join(file_name);
            if target_path.exists() {
                continue;
            }

            let bytes = fs::read(&source_path)?;
            write_atomic_file(&target_path, &bytes)?;
        }

        Ok(())
    }

    fn role_file_path(&self, role_id: &str) -> PathBuf {
        PathBuf::from(ROLES_DIRNAME).join(format!("{role_id}.yaml"))
    }

    fn preset_file_path(&self, preset_id: &str) -> PathBuf {
        PathBuf::from(PRESETS_DIRNAME).join(format!("{preset_id}.yaml"))
    }

    fn apply_single_template_mutation(
        &self,
        mutation: TemplateFileMutation,
        commit_message: &str,
    ) -> Result<TemplateMutationResult, TemplateStoreError> {
        let commit_id = self.mutate_and_commit(&[mutation], commit_message)?;
        Ok(TemplateMutationResult {
            committed: commit_id.is_some(),
            commit_id,
        })
    }

    fn load_role_templates_from_dir(
        &self,
        dir: &Path,
    ) -> Result<Vec<RoleTemplate>, TemplateStoreError> {
        Ok(self
            .load_role_files_from_dir(dir)?
            .into_iter()
            .map(|file| file.template)
            .collect())
    }

    fn load_role_files_from_dir(
        &self,
        dir: &Path,
    ) -> Result<Vec<RoleTemplateFile>, TemplateStoreError> {
        self.load_role_files_from_dir_with(dir, ScanMode::SkipUnparseable)
    }

    fn load_role_files_from_dir_with(
        &self,
        dir: &Path,
        mode: ScanMode,
    ) -> Result<Vec<RoleTemplateFile>, TemplateStoreError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = fs::read_dir(dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        files.sort();

        let mut roles = Vec::new();
        for path in files {
            if !path.is_file() || !is_yaml_file(&path) {
                continue;
            }
            let raw = fs::read_to_string(&path)?;
            let mut parsed = match serde_norway::from_str::<RoleTemplate>(&raw) {
                Ok(parsed) => parsed,
                Err(err) if mode == ScanMode::Strict => {
                    return Err(TemplateStoreError::Parse(format!(
                        "failed to parse role {}: {err}",
                        path.display()
                    )));
                }
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %err,
                        "skipping unparseable role file in directory scan"
                    );
                    continue;
                }
            };
            parsed.normalize_model_fields();
            roles.push(RoleTemplateFile { template: parsed });
        }

        Ok(roles)
    }

    fn load_role_file_by_id(
        &self,
        dir: &Path,
        role_id: &str,
    ) -> Result<Option<RoleTemplateFile>, TemplateStoreError> {
        let path = dir.join(format!("{role_id}.yaml"));
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)?;
        let mut parsed = serde_norway::from_str::<RoleTemplate>(&raw).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to parse role {}: {err}", path.display()))
        })?;
        parsed.normalize_model_fields();
        Ok(Some(RoleTemplateFile { template: parsed }))
    }

    fn load_presets_from_dir(&self, dir: &Path) -> Result<Vec<TeamPreset>, TemplateStoreError> {
        Ok(self
            .load_preset_files_from_dir(dir)?
            .into_iter()
            .map(|file| file.template)
            .collect())
    }

    fn load_preset_files_from_dir(
        &self,
        dir: &Path,
    ) -> Result<Vec<TeamPresetFile>, TemplateStoreError> {
        self.load_preset_files_from_dir_with(dir, ScanMode::SkipUnparseable)
    }

    fn load_preset_files_from_dir_with(
        &self,
        dir: &Path,
        mode: ScanMode,
    ) -> Result<Vec<TeamPresetFile>, TemplateStoreError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = fs::read_dir(dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        files.sort();

        let mut presets = Vec::new();
        for path in files {
            if !path.is_file() || !is_yaml_file(&path) {
                continue;
            }
            let raw = fs::read_to_string(&path)?;
            let mut parsed = match serde_norway::from_str::<TeamPreset>(&raw) {
                Ok(parsed) => parsed,
                Err(err) if mode == ScanMode::Strict => {
                    return Err(TemplateStoreError::Parse(format!(
                        "failed to parse preset {}: {err}",
                        path.display()
                    )));
                }
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %err,
                        "skipping unparseable preset file in directory scan"
                    );
                    continue;
                }
            };
            parsed.normalize_model_fields();
            presets.push(TeamPresetFile { template: parsed });
        }

        Ok(presets)
    }

    fn load_preset_file_by_id(
        &self,
        dir: &Path,
        preset_id: &str,
    ) -> Result<Option<TeamPresetFile>, TemplateStoreError> {
        let path = dir.join(format!("{preset_id}.yaml"));
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)?;
        let mut parsed = serde_norway::from_str::<TeamPreset>(&raw).map_err(|err| {
            TemplateStoreError::Parse(format!("failed to parse preset {}: {err}", path.display()))
        })?;
        parsed.normalize_model_fields();
        Ok(Some(TeamPresetFile { template: parsed }))
    }
}
fn resolve_signature(repo: &Repository) -> Result<Signature<'_>, TemplateStoreError> {
    match repo.signature() {
        Ok(sig) => Ok(sig),
        Err(_) => Signature::now("taurhaus", "templates@local").map_err(TemplateStoreError::Git),
    }
}

fn default_builtins_dir() -> PathBuf {
    // In dev and test builds the repo's resources are the truth: a stale
    // copy under target/debug/resources (deposited by an earlier
    // `tauri dev`/`tauri build`) must not shadow the current catalog —
    // copied artifacts never see deletions, so a consolidation that removes
    // bundled files would silently resurrect them. Packaged builds have no
    // dev dir and keep using the exe-relative resources.
    #[cfg(debug_assertions)]
    {
        let dev = dev_builtins_dir();
        if dev.is_dir() {
            return dev;
        }
    }
    resolve_packaged_builtins_dir().unwrap_or_else(dev_builtins_dir)
}

fn resolve_packaged_builtins_dir() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    packaged_builtins_dir_candidates(&current_exe)
        .into_iter()
        .find(|path| path.is_dir())
}

fn packaged_builtins_dir_candidates(current_exe: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(exe_dir) = current_exe.parent() {
        candidates.push(exe_dir.join("resources").join("templates"));

        if let Some(contents_dir) = exe_dir.parent() {
            candidates.push(
                contents_dir
                    .join("Resources")
                    .join("resources")
                    .join("templates"),
            );
        }
    }

    candidates
}

fn dev_builtins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates")
}

fn current_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

fn parse_mutation_descriptor(message: &str) -> Option<MutationDescriptor> {
    let payload = message.strip_prefix("templates: ")?;
    let mut parts = payload.split_whitespace();
    let action = parts.next()?.to_string();
    let kind = parts.next()?.to_string();
    let id = parts.collect::<Vec<_>>().join(" ");
    if id.trim().is_empty() {
        return None;
    }
    Some(MutationDescriptor { action, kind, id })
}

fn validate_template_id(id: &str, kind: &str) -> Result<(), TemplateStoreError> {
    if id.trim().is_empty() {
        return Err(TemplateStoreError::Validation(format!(
            "{kind} id must not be empty"
        )));
    }

    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(TemplateStoreError::Validation(format!(
            "{kind} id '{id}' must use only [a-zA-Z0-9_-]"
        )));
    }

    Ok(())
}

fn normalize_relative_path(path: &Path) -> Result<PathBuf, TemplateStoreError> {
    if path.is_absolute() {
        return Err(TemplateStoreError::InvalidTemplatePath(format!(
            "absolute paths are not allowed: {}",
            path.display()
        )));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => normalized.push(segment),
            _ => {
                return Err(TemplateStoreError::InvalidTemplatePath(format!(
                    "path contains invalid component: {}",
                    path.display()
                )));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(TemplateStoreError::InvalidTemplatePath(
            "empty relative path".to_string(),
        ));
    }

    Ok(normalized)
}

fn is_managed_template_path(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Normal(first))
            if first == OsStr::new(ROLES_DIRNAME)
                || first == OsStr::new(PRESETS_DIRNAME)
                || first == OsStr::new(META_DIRNAME)
    )
}

fn is_yaml_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("yaml") | Some("yml")
    )
}

fn was_previously_shipped_builtin(relative_path: &Path, bytes: &[u8]) -> bool {
    let digest = format!("{:x}", Sha256::digest(bytes));
    PREVIOUS_BUNDLED_TEMPLATE_HASHES
        .iter()
        .any(|(path, hash)| Path::new(path) == relative_path && *hash == digest)
}

fn insert_preset_role_ids(preset: &TeamPreset, role_ids: &mut BTreeSet<String>) {
    role_ids.insert(preset.lead_role_id.clone());
    role_ids.extend(preset.agent_slots.iter().map(|slot| slot.role_id.clone()));
}

fn temp_path_for(path: &Path) -> PathBuf {
    let random_suffix = format!("{:016x}", rand::thread_rng().next_u64());
    match path.extension().and_then(OsStr::to_str) {
        Some(ext) => path.with_extension(format!("{ext}.tmp.{random_suffix}")),
        None => path.with_extension(format!("tmp.{random_suffix}")),
    }
}

fn is_windows_unsupported_lock_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
}

#[cfg(test)]
fn should_force_fallback_lock_for_tests() -> bool {
    std::env::var_os("TAURHAUS_FORCE_TEMPLATE_LOCK_FALLBACK")
        .map(|value| value == "1")
        .unwrap_or(false)
}

#[cfg(not(test))]
fn should_force_fallback_lock_for_tests() -> bool {
    false
}

fn is_windows_unsupported_rename_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
}

fn acquire_fallback_lock(
    fallback_lock_path: &Path,
) -> Result<FallbackLockGuard, TemplateStoreError> {
    let mut last_conflict = None;
    for _ in 0..FALLBACK_LOCK_RETRY_ATTEMPTS {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(fallback_lock_path)
        {
            Ok(mut file) => {
                let pid = std::process::id();
                let _ = writeln!(file, "{pid}");
                file.sync_all()?;
                return Ok(FallbackLockGuard {
                    path: fallback_lock_path.to_path_buf(),
                    _file: file,
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                last_conflict = Some(err);
                thread::sleep(Duration::from_millis(FALLBACK_LOCK_RETRY_DELAY_MS));
            }
            Err(err) => return Err(TemplateStoreError::Io(err)),
        }
    }

    let cause = last_conflict
        .map(|err| err.to_string())
        .unwrap_or_else(|| "unknown fallback lock contention".to_string());
    Err(TemplateStoreError::LockTimeout(format!(
        "timed out acquiring fallback lock {}: {}",
        fallback_lock_path.display(),
        cause
    )))
}

/// Write `bytes` to `target` through a unique temp file and a rename, so a
/// reader never observes a half-written file. Where the rename itself cannot
/// replace an existing file, `replace_without_atomic_rename` keeps that
/// promise the long way round.
pub(crate) fn write_atomic_file(target: &Path, bytes: &[u8]) -> Result<(), TemplateStoreError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut tmp_open_error = None;
    let mut selected_tmp = None;
    let mut selected_file = None;
    for _ in 0..TEMP_FILE_RANDOM_RETRY_ATTEMPTS {
        let candidate = temp_path_for(target);
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&candidate)
        {
            Ok(file) => {
                selected_tmp = Some(candidate);
                selected_file = Some(file);
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                tmp_open_error = Some(err);
            }
            Err(err) => return Err(TemplateStoreError::Io(err)),
        }
    }

    let tmp = selected_tmp.ok_or_else(|| {
        let err = tmp_open_error.unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "failed to allocate unique temp path after {} attempts for {}",
                    TEMP_FILE_RANDOM_RETRY_ATTEMPTS,
                    target.display()
                ),
            )
        });
        TemplateStoreError::Io(err)
    })?;

    let mut file = selected_file.ok_or_else(|| {
        TemplateStoreError::Io(std::io::Error::other(format!(
            "internal temp-file selection mismatch while writing {}",
            target.display()
        )))
    })?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);

    if let Err(err) = fs::rename(&tmp, target) {
        if is_windows_unsupported_rename_error(&err) {
            tracing::warn!(
                target = %target.display(),
                "atomic rename cannot replace this path; moving the old file aside instead"
            );
            let replaced = replace_without_atomic_rename(&tmp, target);
            let _ = fs::remove_file(&tmp);
            return replaced.map_err(TemplateStoreError::Io);
        }

        let _ = fs::remove_file(&tmp);
        return Err(TemplateStoreError::Io(err));
    }

    Ok(())
}

/// Put `tmp` at `target` on a filesystem whose rename refuses to replace a file
/// that is already there — Windows answers `ERROR_INVALID_FUNCTION` on some
/// WSL-backed and network paths.
///
/// Rewriting `target` in place is not the answer: a reader would see a
/// half-written file and an interruption would leave one on disk, which is the
/// very thing an atomic write exists to prevent. So the old file is moved aside
/// first and the new one renamed into the name it vacated — every state a
/// reader can observe is a whole file, the old one or the new one — and a
/// failure puts the old file back and reports itself rather than claiming a
/// write that did not happen.
fn replace_without_atomic_rename(tmp: &Path, target: &Path) -> std::io::Result<()> {
    let displaced = temp_path_for(target);
    let had_target = match fs::rename(target, &displaced) {
        Ok(()) => true,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => return Err(err),
    };

    match fs::rename(tmp, target) {
        Ok(()) => {
            if had_target {
                let _ = fs::remove_file(&displaced);
            }
            Ok(())
        }
        Err(err) => {
            if had_target {
                if let Err(restore) = fs::rename(&displaced, target) {
                    return Err(std::io::Error::new(
                        err.kind(),
                        format!(
                            "{err}; and restoring the previous file failed ({restore}) — it is at {}",
                            displaced.display()
                        ),
                    ));
                }
            }
            Err(err)
        }
    }
}

fn is_deleted_status(status: Status) -> bool {
    status.intersects(Status::INDEX_DELETED | Status::WT_DELETED | Status::CONFLICTED)
}
