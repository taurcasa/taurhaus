# Round-1 source evidence

[S] Read-only source capture; no taurhaus binary or test suite executed in this source capture.

## Checkout commands

```text
git branch --show-current
main
exit 0
```

```text
git rev-parse HEAD
d4633256027de4988699361dd5288200dc6b3cb7
exit 0
```

## Round-0 hash comparison

[S] SHA-256 computed from bytes; unchanged does not mean tests passed.

| File | Current SHA-256 | Compared with retained hash |
|---|---|---|
| `CLAUDE.md` | `7b62026bd2cae156af6189654fd1913839b00711b457352589e64244348e70ce` | changed; current instructions read anew |
| `docs/design/onboarding-card-brief.md` | `122aca1c773ce92eb432a9796dbb13709f99f89d3be834b41bbad02bde5783d0` | unchanged |
| `src-tauri/src/coordination/delivery.rs` | `7f4375d2d81e19e1994b661e9171b1b8d12123f31c977359a54b8a57b0cafa40` | unchanged |
| `src-tauri/src/coordination/reinjection.rs` | `db1aab025ea05bf1abaa5aa12b91ff9d4618c8307538df1fd9060c0e264cc90b` | unchanged |
| `src-tauri/src/coordination/pipelines/lifecycle.rs` | `d706016bce9d838fb113e62546fdc97cd947b1627eaa128173b8168092f80d45` | unchanged |
| `src-tauri/src/coordination/pipelines/members.rs` | `2b26adf09cd6422365137edb31d8aea1a66576c94164e3a3ae911030f5243628` | unchanged |
| `src-tauri/src/coordination/pipelines/effort.rs` | `580f356386cbc34ce7b978f27372e29f63856bfb0c009e13713e3281ae5ff3a2` | unchanged |
| `src-tauri/src/coordination/compact_hook.rs` | `928306ab2dd5cc4a8c24511eb6a0e5dd738cfd6c22a88eea9fffe489d325f541` | unchanged |
| `src-tauri/src/daemon/team_runs.rs` | `35fd3265955e6c618ab52fba677b688970cc337fa390284fc2985174684afb49` | unchanged |
| `src-tauri/resources/mesh.lock.json` | `c35e575b4aeea063a3030533b716235e3038270f6fc050a649aa80892f521116` | unchanged |
| `/home/mstie/.local/bin/mesh` | `408c7e0cc7ed5302f4e5fb9a2be3b8e47f75b4c14ab9e2127e6bad4572f2f672` | unchanged |

## Source excerpts

### src-tauri/src/daemon/team_move.rs:18–100

```text
18: pub(crate) fn move_team_directory(
19:     source_teams: &Path,
20:     target_teams: &Path,
21:     team_name: &str,
22: ) -> Result<TeamMoveStrategy, CoordinationError> {
23:     move_team_directory_with(source_teams, target_teams, team_name, &mut |from, to| {
24:         fs::rename(from, to)
25:     })
26: }
27: 
28: fn move_team_directory_with(
29:     source_teams: &Path,
30:     target_teams: &Path,
31:     team_name: &str,
32:     rename: &mut impl FnMut(&Path, &Path) -> std::io::Result<()>,
33: ) -> Result<TeamMoveStrategy, CoordinationError> {
34:     let source = source_teams.join(team_name);
35:     let target = target_teams.join(team_name);
36:     if !source.is_dir() {
37:         return Err(CoordinationError::NotFound(format!(
38:             "team directory not found at '{}'",
39:             source.display()
40:         )));
41:     }
42:     if target.exists() {
43:         return Err(CoordinationError::Validation(format!(
44:             "target team directory already exists at '{}'",
45:             target.display()
46:         )));
47:     }
48:     fs::create_dir_all(target_teams)?;
49:     match rename(&source, &target) {
50:         Ok(()) => return Ok(TeamMoveStrategy::Rename),
51:         Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {}
52:         Err(error) => return Err(CoordinationError::Io(error)),
53:     }
54: 
55:     let nonce = uuid::Uuid::new_v4().simple().to_string();
56:     let staging = target_teams.join(format!(".{team_name}.taurhaus-move-{nonce}"));
57:     let backup = source_teams.join(format!(".{team_name}.taurhaus-backup-{nonce}"));
58:     let expected = snapshot_tree(&source)?;
59:     if let Err(error) = copy_tree(&source, &staging).and_then(|()| verify_tree(&staging, &expected))
60:     {
61:         remove_dir_if_present(&staging);
62:         return Err(error);
63:     }
64: 
65:     if let Err(error) = rename(&source, &backup) {
66:         remove_dir_if_present(&staging);
67:         return Err(CoordinationError::Io(error));
68:     }
69:     if let Err(error) = rename(&staging, &target) {
70:         let restore = rename(&backup, &source);
71:         remove_dir_if_present(&staging);
72:         return match restore {
73:             Ok(()) => Err(CoordinationError::Io(error)),
74:             Err(restore_error) => Err(CoordinationError::StoreError(format!(
75:                 "team move promotion failed ({error}); source restore failed ({restore_error})"
76:             ))),
77:         };
78:     }
79: 
80:     if let Err(error) = fs::remove_dir_all(&backup) {
81:         tracing::warn!(
82:             path = %backup.display(),
83:             error = %error,
84:             "verified team move left a hidden source backup"
85:         );
86:     }
87:     Ok(TeamMoveStrategy::CopyVerify)
88: }
89: 
90: fn copy_tree(source: &Path, target: &Path) -> Result<(), CoordinationError> {
91:     fs::create_dir(target)?;
92:     for entry in fs::read_dir(source)? {
93:         let entry = entry?;
94:         let file_type = entry.file_type()?;
95:         let destination = target.join(entry.file_name());
96:         if file_type.is_dir() {
97:             copy_tree(&entry.path(), &destination)?;
98:         } else if file_type.is_file() {
99:             fs::copy(entry.path(), destination)?;
100:         } else {
```

### src-tauri/src/daemon/team_runs.rs:533–587

```text
533:         orchestrator.stop_team_daemon_best_effort(&request.team_name);
534:         for member in &config.members {
535:             orchestrator
536:                 .stop_member_for_account_switch(&request.team_name, member)
537:                 .map_err(CoordinationError::Backend)?;
538:         }
539: 
540:         let previous_config = config.clone();
541:         for member in &mut config.members {
542:             if member.cli_tool == request.cli_tool {
543:                 member.account_id = Some(target.id.clone());
544:             }
545:         }
546:         let manifest = AccountSwitchHandoffManifest {
547:             switched_at: chrono::Utc::now(),
548:             cli_tool: request.cli_tool,
549:             account_id: target.id.clone(),
550:             account_label: target.label.clone(),
551:             members: handoffs.clone(),
552:             team_state_move: team_root_switch.then(|| TeamStateMove {
553:                 from_teams_dir: teams_dir.display().to_string(),
554:                 to_teams_dir: target_teams_dir.display().to_string(),
555:                 strategy: "rename-or-copy-verify".to_string(),
556:             }),
557:         };
558:         TeamConfigStore::save(&teams_dir, &request.team_name, &config)?;
559: 
560:         if team_root_switch {
561:             if let Err(error) = crate::daemon::team_move::move_team_directory(
562:                 &teams_dir,
563:                 &target_teams_dir,
564:                 &request.team_name,
565:             ) {
566:                 TeamConfigStore::save(&teams_dir, &request.team_name, &previous_config)?;
567:                 return Err(error);
568:             }
569:             if let Err(error) = state
570:                 .team_root_registry()
571:                 .set(&request.team_name, &target_teams_dir)
572:             {
573:                 let rollback = crate::daemon::team_move::move_team_directory(
574:                     &target_teams_dir,
575:                     &teams_dir,
576:                     &request.team_name,
577:                 );
578:                 if rollback.is_ok() {
579:                     TeamConfigStore::save(&teams_dir, &request.team_name, &previous_config)?;
580:                 }
581:                 return match rollback {
582:                     Ok(_) => Err(error),
583:                     Err(rollback_error) => Err(CoordinationError::StoreError(format!(
584:                         "team root registry commit failed ({error}); move rollback failed ({rollback_error})"
585:                     ))),
586:                 };
587:             }
```

### src-tauri/src/coordination/compact_hook.rs:512–573

```text
512:     let Some(snapshot) = OperationalContextSnapshotStore::load(
513:         &matched.teams_dir,
514:         &matched.team_name,
515:         &matched.member.name,
516:     )?
517:     else {
518:         record_delivery_at(
519:             &matched.teams_dir,
520:             &matched.team_name,
521:             &matched.member.name,
522:             tool,
523:             &payload.session_id,
524:             compaction_timestamp,
525:             CompactionDeliveryResult::Skipped,
526:         )
527:         .inspect_err(|error| {
528:             emit_compact_hook_failed(
529:                 CompactHookFailureStage::RecordDelivery,
530:                 Some(&payload),
531:                 Some(&matched),
532:                 None,
533:                 None,
534:                 None,
535:                 &error.to_string(),
536:             );
537:         })?;
538:         emit_compact_hook_skipped(
539:             &payload,
540:             Some(&matched),
541:             CompactHookSkipReason::MissingOperationalSnapshot,
542:         );
543:         return Ok(CompactHookResponse::default());
544:     };
545: 
546:     if !CompactionReinjectionService::snapshot_has_resumable_task(&snapshot) {
547:         record_delivery_at(
548:             &matched.teams_dir,
549:             &matched.team_name,
550:             &matched.member.name,
551:             tool,
552:             &payload.session_id,
553:             compaction_timestamp,
554:             CompactionDeliveryResult::Skipped,
555:         )
556:         .inspect_err(|error| {
557:             emit_compact_hook_failed(
558:                 CompactHookFailureStage::RecordDelivery,
559:                 Some(&payload),
560:                 Some(&matched),
561:                 None,
562:                 None,
563:                 None,
564:                 &error.to_string(),
565:             );
566:         })?;
567:         emit_compact_hook_skipped(
568:             &payload,
569:             Some(&matched),
570:             CompactHookSkipReason::NoResumableTaskContext,
571:         );
572:         return Ok(CompactHookResponse::default());
573:     }
```

### src-tauri/src/coordination/compact_hook.rs:626–657

```text
626:     record_delivery_at(
627:         &matched.teams_dir,
628:         &matched.team_name,
629:         &matched.member.name,
630:         tool,
631:         &payload.session_id,
632:         compaction_timestamp,
633:         CompactionDeliveryResult::Injected,
634:     )
635:     .inspect_err(|error| {
636:         emit_compact_hook_failed(
637:             CompactHookFailureStage::RecordDelivery,
638:             Some(&payload),
639:             Some(&matched),
640:             None,
641:             None,
642:             None,
643:             &error.to_string(),
644:         );
645:     })?;
646: 
647:     emit_compact_hook_delivered(&payload, &matched, additional_context.len());
648: 
649:     Ok(match delivery {
650:         CompactionDelivery::HookStdout => CompactHookResponse {
651:             hook_specific_output: Some(CompactHookSpecificOutput {
652:                 hook_event_name: SESSION_START_HOOK_EVENT.to_string(),
653:                 additional_context,
654:             }),
655:         },
656:         CompactionDelivery::MeshInbox => CompactHookResponse::default(),
657:     })
```

### src-tauri/src/coordination/stores/compaction.rs:15–40

```text
15: #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
16: #[serde(rename_all = "snake_case")]
17: pub enum CompactionDeliveryResult {
18:     Injected,
19:     #[serde(alias = "stale")]
20:     Skipped,
21:     Failed,
22: }
23: 
24: #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
25: #[serde(rename_all = "snake_case")]
26: pub struct MemberCompactionState {
27:     pub version: u32,
28:     pub member_name: String,
29:     pub last_session_id: String,
30:     pub last_compaction_timestamp: DateTime<Utc>,
31:     pub last_delivery_result: CompactionDeliveryResult,
32: }
33: 
34: #[derive(Debug, Default)]
35: pub struct MemberCompactionStore;
36: 
37: impl MemberCompactionStore {
38:     pub fn load(
39:         teams_dir: &Path,
40:         team_name: &str,
```

### src-tauri/src/templates/agent_definitions.rs:47–55

```text
47:     rendered.push_str("---\n\n");
48:     rendered.push_str(GENERATED_MARKER);
49:     rendered.push('\n');
50: 
51:     let body = DeliveryRenderer::render_role_sections(&RoleContext::from(&role));
52:     if !body.is_empty() {
53:         rendered.push('\n');
54:         rendered.push_str(&body);
55:         rendered.push('\n');
```

### src-tauri/src/coordination/delivery.rs:101–109

```text
101:                 "Work contract:\n",
102:                 "Execute the first action; report through the named completion signal. Record an explicit dependency wait when execution cannot begin.\n",
103:                 "\n",
104:                 "Compaction safety:\n",
105:                 "If context compaction happens and you have no unread messages or your current task is unclear, immediately message {lead_name} and ask for your current assignment.\n",
106:                 "Do not assume you are done — compaction may have dropped your active task context.\n",
107:                 "\n",
108:                 "Escalation:\n",
109:                 "If blocked, send blocker details to {lead_name} immediately. Do not stall silently."
```

### src-tauri/src/coordination/delivery.rs:136–145

```text
136:                 "Work contract:\n",
137:                 "Do the assigned work first, then report completion with artifacts and test results. Record an explicit dependency wait when execution cannot begin.\n",
138:                 "Do not send a pure acknowledgment before you have either completed the work or identified a real blocker.\n",
139:                 "\n",
140:                 "Compaction safety:\n",
141:                 "If context compaction happens and you have no unread messages or your current task is unclear, immediately message {lead_name} and ask for your current assignment.\n",
142:                 "Do not assume you are done — compaction may have dropped your active task context.\n",
143:                 "\n",
144:                 "Escalation:\n",
145:                 "If blocked, send blocker details to {lead_name} immediately. Do not stall silently."
```

### src-tauri/src/coordination/delivery.rs:188–229

```text
188:     /// The role steering text that follows every onboarding contract. It is
189:     /// also the body of a generated Claude Code agent definition, so a role
190:     /// steers a mesh member and a subagent with the very same words.
191:     pub fn render_role_sections(role_context: &RoleContext<'_>) -> String {
192:         let mut blocks: Vec<String> = Vec::new();
193: 
194:         if let Some(role_id) = Self::trimmed(role_context.role_id) {
195:             blocks.push(format!("Role: {role_id}"));
196:         }
197: 
198:         if let Some(communication_style) = Self::trimmed(role_context.communication_style) {
199:             blocks.push(format!("Communication Style:\n{communication_style}"));
200:         }
201: 
202:         if let Some(instructions) = Self::trimmed(role_context.instructions) {
203:             blocks.push(format!("Instructions:\n{instructions}"));
204:         }
205: 
206:         if let Some(contract) = role_context.behavioral_contract.filter(|contract| {
207:             !contract.communication.is_empty()
208:                 || !contract.execution.is_empty()
209:                 || !contract.escalation.is_empty()
210:         }) {
211:             let mut block = String::from("Behavioral Contract:");
212:             Self::append_titled_bullets(&mut block, "Communication", &contract.communication);
213:             Self::append_titled_bullets(&mut block, "Execution", &contract.execution);
214:             Self::append_titled_bullets(&mut block, "Escalation", &contract.escalation);
215:             blocks.push(block);
216:         }
217: 
218:         for (title, items) in [
219:             ("Quality Gates", role_context.quality_gates),
220:             ("Handoff Expectations", role_context.handoff_expectations),
221:             ("Definition of Done", role_context.definition_of_done),
222:             ("Capabilities", role_context.capabilities),
223:         ] {
224:             let Some(items) = items.filter(|items| Self::has_non_empty_items(items)) else {
225:                 continue;
226:             };
227:             let mut block = format!("{title}:\n");
228:             Self::append_bullets(&mut block, items);
229:             blocks.push(block);
```

### src-tauri/tests/cli_renderers.rs:379–462

```text
379: fn render_onboarding_cli_matches_delivery_renderer_bytes() {
380:     // Regression: commit 7b852ed copied only part of DeliveryRenderer into
381:     // taureval, dropping workflow fields whenever the taurhaus role evolved.
382:     let role_yaml = include_str!("../resources/templates/roles/quick-dev-codex.yaml");
383:     let role: RoleTemplate = serde_norway::from_str(role_yaml).expect("bundled role parses");
384:     let role_wire: serde_norway::Value =
385:         serde_norway::from_str(role_yaml).expect("bundled role parses as wire value");
386: 
387:     for (tool, expected, golden) in [
388:         (
389:             "codex",
390:             DeliveryRenderer::render_onboarding(
391:                 "taureval-golden",
392:                 "agent-under-test",
393:                 "evaluator",
394:                 RoleContext::from(&role),
395:             ),
396:             include_str!("quick-dev-codex-onboarding.golden.txt"),
397:         ),
398:         (
399:             "claude",
400:             DeliveryRenderer::render_claude_role_context(
401:                 "taureval-golden",
402:                 "agent-under-test",
403:                 "evaluator",
404:                 RoleContext::from(&role),
405:             ),
406:             include_str!("quick-dev-claude-onboarding.golden.txt"),
407:         ),
408:     ] {
409:         let request = serde_json::json!({
410:             "tool": tool,
411:             "team_name": "taureval-golden",
412:             "member_name": "agent-under-test",
413:             "lead_name": "evaluator",
414:             "role": role_wire.clone()
415:         });
416:         let actual = run_renderer("--render-onboarding", &request);
417: 
418:         assert_eq!(actual, format!("{expected}\n"));
419:         assert_eq!(actual, golden);
420:     }
421: }
422: 
423: #[test]
424: fn render_onboarding_cli_uses_the_agy_variant() {
425:     // Regression: commit ac6f006 exposed one generic renderer CLI, so adding
426:     // Antigravity without selecting its variant omitted `/exit` and the inbox.
427:     let role_yaml = include_str!("../resources/templates/roles/quick-dev-codex.yaml");
428:     let role_wire: serde_norway::Value =
429:         serde_norway::from_str(role_yaml).expect("bundled role parses as wire value");
430:     let request = serde_json::json!({
431:         "tool": "agy",
432:         "team_name": "taureval-golden",
433:         "member_name": "agent-under-test",
434:         "lead_name": "evaluator",
435:         "role": role_wire
436:     });
437:     let actual = run_renderer("--render-onboarding", &request);
438: 
439:     // Regression: 18810949 moved teams to account roots; the inbox hint must follow the launch root.
440:     assert!(actual.contains("$CLAUDE_DIR/teams/taureval-golden/inboxes/agent-under-test.json"));
441:     assert!(actual.contains("enter /exit"));
442:     // Regression: agy loads hooks only in a trusted workspace, so an onboarded
443:     // member who never answers the trust prompt reports no activity at all.
444:     assert!(actual.contains("trust"));
445:     assert!(actual.contains("first launch"));
446: }
447: 
448: #[test]
449: fn render_onboarding_cli_uses_the_grok_variant() {
450:     // Regression: commit bfecae9 had no grok registry entry, so the shared
451:     // renderer CLI could not select its `/quit`, inbox and queueing-Enter text.
452:     let role_yaml = include_str!("../resources/templates/roles/quick-dev-codex.yaml");
453:     let role_wire: serde_norway::Value =
454:         serde_norway::from_str(role_yaml).expect("bundled role parses as wire value");
455:     let request = serde_json::json!({
456:         "tool": "grok",
457:         "team_name": "taureval-golden",
458:         "member_name": "agent-under-test",
459:         "lead_name": "evaluator",
460:         "role": role_wire
461:     });
462:     let actual = run_renderer("--render-onboarding", &request);
```

### src-tauri/src/session_scanner/cli_tool.rs:454–459

```text
454:                 native_inbox_poller: false,
455:                 session_source: true,
456:                 runtime_session_capture: false,
457:                 authoritative_idle: false,
458:                 compaction_hook: false,
459:                 compaction_delivery: CompactionDelivery::HookStdout,
```

### src-tauri/src/coordination/task_deadline_pass.rs:248–257

```text
248:         DeadlineAction::Nudge => orchestrator
249:             .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
250:                 team_name: team_name.to_string(),
251:                 member_name: member_name.to_string(),
252:                 message: format!(
253:                     "ACTION REQUIRED: Task #{} — half the deadline is gone ({} minutes total); report progress or BLOCKED.",
254:                     snapshot.task.id, deadline_minutes
255:                 ),
256:                 sender_name: sender_name.map(ToString::to_string),
257:                 operational_context: None,
```

### docs/architecture/harness-model.md:102–102

```text
102: Usage is a second provider slice attached to each detected account as an in-memory snapshot. Providers read native state at request time; taurhaus never logs, persists, refreshes, or otherwise owns a credential. Claude uses `CLAUDE_CONFIG_DIR` and its OAuth usage endpoint. Codex uses `CODEX_HOME`, display-only decoding of the `id_token`, and its native usage windows; API-key accounts remain selectable but explicitly report usage as unavailable. Antigravity exposes one implicit account and obtains its native windows by running `agy -p /usage --output-format json` through the injectable command boundary. Grok uses `GROK_HOME` and reads only the display names in its `auth.json`; it reports usage as unavailable because grok 1.0.5 publishes no quota endpoint, and the registry carries the sentence the UI shows in a meter's place. The retired Claude status-line bridge is uninstalled once without disturbing foreign status-line commands.
```

### src-tauri/resources/mesh.lock.json:1–6

```text
1: {
2:   "version": "0.2.29",
3:   "protocol_version": 1,
4:   "schema_version": 1,
5:   "git_commit": "6789201c5511b51be704fe30c6e4d025f3e64f8c"
6: }
```
