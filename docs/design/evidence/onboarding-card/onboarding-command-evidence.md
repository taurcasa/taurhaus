# Isolated Mesh command evidence

Every subprocess used environment={}, cwd=/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7, and the absolute binary plus explicit --claude-dir below. No daemon, watch, timer, harness, or tmux command was started.

## C1

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 --version
```
Exit: 0
Stdout (verbatim):
```text
mesh 0.2.29
```
Stderr (verbatim):
```text

```

## C2

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 --help
```
Exit: 0
Stdout (verbatim):
```text
Filesystem-based IPC for co-located AI agents

Usage: mesh [OPTIONS] <COMMAND>

Commands:
  join          Register presence and create inbox
  leave         Remove presence (set inactive)
  send          Send a direct message to another agent's inbox
  broadcast     Broadcast a message to all other active team members
  xteam         Lead-only cross-team operator commands
  ack           Acknowledge a received message by ID
  ack-status    Query acknowledgment status for a message ID
  read          Read inbox messages
  who           List online team members
  heartbeat     Record an explicit activity heartbeat for this member
  status        Set/get explicit member status for stall detection suppression
  team-daemon   Team-level background daemon for orchestration concerns
  nudge         Send a generated actionable nudge for an owner/task pair
  version       Show mesh build/version metadata
  tasks         List tasks
  task          Task sub-operations
  lease         Seam lease sub-operations
  watch         Watch inbox for new messages
  daemon        Watch inbox and deliver messages to a tmux pane
  timer         Set a delayed message delivery timer
  timer-cancel  Cancel all timers for the team
  help          Print this message or the help of the given subcommand(s)

Options:
      --team <TEAM>              Team name [env: MESH_TEAM=]
      --name <NAME>              Agent name (your handle) [env: MESH_NAME=]
      --claude-dir <CLAUDE_DIR>  Claude config directory [env: CLAUDE_DIR=]
  -h, --help                     Print help
  -V, --version                  Print version
```
Stderr (verbatim):
```text

```

## C3

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 team create --help
```
Exit: 2
Stdout (verbatim):
```text

```
Stderr (verbatim):
```text
error: unrecognized subcommand 'team'

  tip: some similar subcommands exist: 'team-daemon', 'xteam'

Usage: mesh [OPTIONS] <COMMAND>

For more information, try '--help'.
```

## C4

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 join --help
```
Exit: 0
Stdout (verbatim):
```text
Register presence and create inbox

Usage: mesh join [OPTIONS]

Options:
      --team <TEAM>              Team name (required, overrides global --team)
      --name <NAME>              Agent name (required, overrides global --name)
      --claude-dir <CLAUDE_DIR>  Claude config directory [env: CLAUDE_DIR=]
      --type <TYPE>              Agent type [default: general-purpose]
      --model <MODEL>            Model name [default: external]
      --color <COLOR>            Display color
  -h, --help                     Print help
```
Stderr (verbatim):
```text

```

## C5

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 send --help
```
Exit: 0
Stdout (verbatim):
```text
Send a direct message to another agent's inbox

Usage: mesh send [OPTIONS] <RECIPIENT> <MESSAGE>

Arguments:
  <RECIPIENT>  Recipient agent name
  <MESSAGE>    Message text

Options:
      --summary <SUMMARY>        Short summary for UI preview
      --team <TEAM>              Team name [env: MESH_TEAM=]
      --name <NAME>              Agent name (your handle) [env: MESH_NAME=]
      --priority <PRIORITY>      Message priority: urgent (default) or low [default: urgent]
      --claude-dir <CLAUDE_DIR>  Claude config directory [env: CLAUDE_DIR=]
  -h, --help                     Print help
```
Stderr (verbatim):
```text

```

## C6

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 task create --help
```
Exit: 0
Stdout (verbatim):
```text
Create a new task

Usage: mesh task create [OPTIONS] --subject <SUBJECT>

Options:
      --subject <SUBJECT>
          Task subject
      --team <TEAM>
          Team name [env: MESH_TEAM=]
      --json
          Output the created task record as JSON
      --name <NAME>
          Agent name (your handle) [env: MESH_NAME=]
      --claude-dir <CLAUDE_DIR>
          Claude config directory [env: CLAUDE_DIR=]
      --description <DESCRIPTION>
          Task description [default: ]
      --active-form <ACTIVE_FORM>
          Active form (present-continuous, e.g. "Running tests")
      --first-step <FIRST_STEP>
          Required first action for the assignee
      --deliverable <DELIVERABLE>
          Required deliverable or output contract
      --completion-signal <COMPLETION_SIGNAL>
          Completion signal for the assignment
      --lane-id <LANE_ID>
          Orchestration lane identifier
      --work-kind <WORK_KIND>
          Work kind: runtime, verification, scaffolding, docs, review
      --criticality <CRITICALITY>
          Criticality: `critical_path`, supporting, `scaffolding_only`
      --parent <PARENT>
          Parent task ID for scaffold anchoring
      --anchor <ANCHOR>
          Anchor task ID for scaffold anchoring
      --scaffold-class <SCAFFOLD_CLASS>
          Scaffold class: checklist, handoff, wrapper, `fixture_note`, `implementation_note`, none
      --sunset-decision <SUNSET_DECISION>
          Sunset decision: merge, archive, delete, `not_applicable`
      --sunset-owner <SUNSET_OWNER>
          Sunset owner
      --sunset-trigger <SUNSET_TRIGGER>
          Sunset trigger
      --effort <EFFORT>
          Runtime effort the assignee should work at: low, medium, high, xhigh
      --why <WHY>
          Why that effort level fits this task (one line)
      --deadline <DEADLINE>
          Deadline in positive whole minutes
  -h, --help
          Print help
```
Stderr (verbatim):
```text

```

## C7

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 task assign --help
```
Exit: 0
Stdout (verbatim):
```text
Assign a task to an owner and send generated actionable instructions

Usage: mesh task assign [OPTIONS] --owner <OWNER> <ID>

Arguments:
  <ID>  Task ID

Options:
      --owner <OWNER>
          Owner/member name
      --team <TEAM>
          Team name [env: MESH_TEAM=]
      --name <NAME>
          Agent name (your handle) [env: MESH_NAME=]
      --reopen
          Explicitly reopen a completed task with a fresh assignment (lead-only; requires --admin-reason)
      --awaiting-go
          Wait for explicit GO before idle nudges (lead-only; clear with task update --go)
      --claude-dir <CLAUDE_DIR>
          Claude config directory [env: CLAUDE_DIR=]
      --status <STATUS>
          Assignment status (`pending` or `in_progress`) [default: pending]
      --first-step <FIRST_STEP>
          Required first action for the assignee
      --deliverable <DELIVERABLE>
          Required deliverable or output contract
      --completion-signal <COMPLETION_SIGNAL>
          Completion signal for the assignment
      --override-lane-limit
          Explicitly bypass the one-active-lane-per-person guardrail
      --override-reason <OVERRIDE_REASON>
          Durable reason for bypassing the lane guardrail
      --admin-reason <ADMIN_REASON>
          Explicit admin reason when superseding an existing owner or assignment
      --effort <EFFORT>
          Optional runtime effort override for the assignee: low, medium, high, xhigh
      --why <WHY>
          Optional reason for the chosen effort level (one line)
      --deadline <DEADLINE>
          Deadline in positive whole minutes
  -h, --help
          Print help
```
Stderr (verbatim):
```text

```

## C8

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 task get --help
```
Exit: 0
Stdout (verbatim):
```text
Get a task by ID

Usage: mesh task get [OPTIONS] <ID>

Arguments:
  <ID>  Task ID

Options:
      --json                     Output as JSON
      --team <TEAM>              Team name [env: MESH_TEAM=]
      --name <NAME>              Agent name (your handle) [env: MESH_NAME=]
      --verbose                  Show the full pretty-printed task record
      --claude-dir <CLAUDE_DIR>  Claude config directory [env: CLAUDE_DIR=]
  -h, --help                     Print help
```
Stderr (verbatim):
```text

```

## C9

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 read --help
```
Exit: 0
Stdout (verbatim):
```text
Read inbox messages

Usage: mesh read [OPTIONS]

Options:
      --team <TEAM>              Team name [env: MESH_TEAM=]
      --unread                   Show only unread messages
      --last <LAST>              Number of messages to show [default: 20]
      --name <NAME>              Agent name (your handle) [env: MESH_NAME=]
      --claude-dir <CLAUDE_DIR>  Claude config directory [env: CLAUDE_DIR=]
      --json                     Output as JSON
      --mark-read                Mark displayed messages as read
      --short                    Truncate message text to 120 characters
      --guide                    Show the read-decision guide footer
      --priority <PRIORITY>      Filter by priority (urgent or low)
  -h, --help                     Print help
```
Stderr (verbatim):
```text

```

## C10

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 version --json
```
Exit: 0
Stdout (verbatim):
```text
{
  "version": "0.2.29",
  "git_commit": "6789201c5511b51be704fe30c6e4d025f3e64f8c",
  "git_dirty": false,
  "build_time_utc": "2026-09-07T13:25:52Z",
  "protocol_version": 1,
  "schema_version": 1
}
```
Stderr (verbatim):
```text

```

## C11

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 join --team card-fixture --name lead --type lead
```
Exit: 0
Stdout (verbatim):
```text
[mesh] lead joined team card-fixture
```
Stderr (verbatim):
```text
bootstrapped new team config
```

## C12

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 join --team card-fixture --name seat
```
Exit: 0
Stdout (verbatim):
```text
[mesh] seat joined team card-fixture
```
Stderr (verbatim):
```text

```

## C13

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 join --team card-fixture --name seat
```
Exit: 0
Stdout (verbatim):
```text
[mesh] seat joined team card-fixture
```
Stderr (verbatim):
```text
already a member of this team, updating presence
```

## C14

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 read --team card-fixture --name seat --unread --json
```
Exit: 0
Stdout (verbatim):
```text
[]
```
Stderr (verbatim):
```text

```

## C15

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 send seat 'INFO ONLY: Card reference changed; no response needed' --summary 'card reference' --priority low --team card-fixture --name lead
```
Exit: 0
Stdout (verbatim):
```text
[mesh] lead -> seat: sent [low] (id: a1f46e63-1ab1-4a8b-af73-4d52172f8c4d)
```
Stderr (verbatim):
```text

```

## C16

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 read --team card-fixture --name seat --unread --mark-read --json
```
Exit: 0
Stdout (verbatim):
```text
[
  {
    "id": "a1f46e63-1ab1-4a8b-af73-4d52172f8c4d",
    "from": "lead",
    "text": "INFO ONLY: Card reference changed; no response needed\n\n[orchestration_v1]\nintent: info\nno_response_needed: true\n[/orchestration_v1]",
    "timestamp": "2026-09-08T02:11:56.696Z",
    "read": false,
    "summary": "card reference",
    "priority": "low"
  }
]
```
Stderr (verbatim):
```text

```

## C17

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 task create --subject 'Fixture card recovery' --description 'Scratch-only command contract' --first-step 'Inspect fixture.txt' --deliverable 'Fixture result' --completion-signal 'Report fixture result' --team card-fixture --name lead --json
```
Exit: 0
Stdout (verbatim):
```text
{
  "id": "1",
  "subject": "Fixture card recovery",
  "description": "Scratch-only command contract",
  "status": "pending",
  "metadata": {
    "completion_signal": "Report fixture result",
    "deliverable": "Fixture result",
    "first_step": "Inspect fixture.txt"
  }
}
```
Stderr (verbatim):
```text

```

## C18

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 task assign 1 --owner seat --awaiting-go --team card-fixture --name lead
```
Exit: 0
Stdout (verbatim):
```text
[mesh] assigned task #1 -> seat (pending) (id: 5ed1d397-42e8-43c2-b006-cbdfc7b18509)
```
Stderr (verbatim):
```text

```

## C19

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 task get 1 --team card-fixture --name seat
```
Exit: 0
Stdout (verbatim):
```text
#1 [pending] Fixture card recovery
Lifecycle: assigned
Owner: seat
Assignment: 5dfe4b79-40f7-487b-9cf9-dd9e0458f730
Counts as active lane: no
Description: Scratch-only command contract
First step: Inspect fixture.txt
Deliverable: Fixture result
Completion: Report fixture result
Metadata keys: assigned_at, assigned_by, awaiting_go
Use --json or --verbose for the full task record.
```
Stderr (verbatim):
```text

```

## C20

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 read --team card-fixture --name seat --unread --mark-read --json
```
Exit: 0
Stdout (verbatim):
```text
[
  {
    "id": "5ed1d397-42e8-43c2-b006-cbdfc7b18509",
    "from": "lead",
    "text": "[#1 Fixture card recovery · owner: seat]\nACTION REQUIRED: Task #1 is yours but WAITS FOR GO: Fixture card recovery.\nTask ID: #1\nAssignment: 5dfe4b79-40f7-487b-9cf9-dd9e0458f730\nDo not start: the lead releases this task with `mesh task update 1 --go`. You may pre-read context now.\nFirst step after GO: Inspect fixture.txt\nDeliverable: Fixture result\nOnly reply after: Report fixture result\nDo not send an acknowledgment or task summary before doing the work.",
    "timestamp": "2026-09-08T02:11:56.746Z",
    "read": false,
    "summary": "task #1 assignment",
    "taskId": "1",
    "taskSubject": "Fixture card recovery",
    "kind": "assignment"
  }
]
```
Stderr (verbatim):
```text

```

## C21

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 task get 1 --json --team card-fixture --name seat
```
Exit: 0
Stdout (verbatim):
```text
{
  "id": "1",
  "subject": "Fixture card recovery",
  "description": "Scratch-only command contract",
  "status": "pending",
  "owner": "seat",
  "metadata": {
    "assigned_at": "2026-09-08T02:11:56.746Z",
    "assigned_by": "lead",
    "assignment_id": "5dfe4b79-40f7-487b-9cf9-dd9e0458f730",
    "awaiting_go": "5dfe4b79-40f7-487b-9cf9-dd9e0458f730",
    "completion_signal": "Report fixture result",
    "deliverable": "Fixture result",
    "first_step": "Inspect fixture.txt"
  },
  "lifecycleStage": "assigned",
  "countsAsActiveLane": false,
  "pendingEffort": false
}
```
Stderr (verbatim):
```text

```

## C22

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 nudge seat --task 1 --team card-fixture --name lead
```
Exit: 2
Stdout (verbatim):
```text

```
Stderr (verbatim):
```text
error: unexpected argument 'seat' found

Usage: mesh nudge [OPTIONS] --owner <OWNER> --task <TASK>

For more information, try '--help'.
```

## C23

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 read --unread --mark-read --json --team card-fixture --name seat
```
Exit: 0
Stdout (verbatim):
```text
[]
```
Stderr (verbatim):
```text

```

## C24

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 nudge --owner seat --task 1 --team card-fixture --name lead
```
Exit: 0
Stdout (verbatim):
```text
[mesh] nudged seat for task #1 (id: 672180d2-db89-4003-9d01-1a166d297a56)
```
Stderr (verbatim):
```text

```

## C25

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 read --unread --mark-read --json --team card-fixture --name seat
```
Exit: 0
Stdout (verbatim):
```text
[
  {
    "id": "672180d2-db89-4003-9d01-1a166d297a56",
    "from": "lead",
    "text": "[#1 Fixture card recovery · owner: seat]\nACTION REQUIRED: Resume task #1 now: Fixture card recovery.\nTask ID: #1\nStart now: Inspect fixture.txt\nDeliverable: Fixture result\nOnly reply after: Report fixture result\nDo not send an acknowledgment or task summary before doing the work.",
    "timestamp": "2026-09-08T02:14:08.485Z",
    "read": false,
    "summary": "idle nudge task #1",
    "taskId": "1",
    "taskSubject": "Fixture card recovery",
    "kind": "nudge"
  }
]
```
Stderr (verbatim):
```text

```

## C26

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 task get 1 --json --team card-fixture --name seat
```
Exit: 0
Stdout (verbatim):
```text
{
  "id": "1",
  "subject": "Fixture card recovery",
  "description": "Scratch-only command contract",
  "status": "pending",
  "owner": "seat",
  "metadata": {
    "assigned_at": "2026-09-08T02:11:56.746Z",
    "assigned_by": "lead",
    "assignment_id": "5dfe4b79-40f7-487b-9cf9-dd9e0458f730",
    "awaiting_go": "5dfe4b79-40f7-487b-9cf9-dd9e0458f730",
    "completion_signal": "Report fixture result",
    "deliverable": "Fixture result",
    "first_step": "Inspect fixture.txt"
  },
  "lifecycleStage": "assigned",
  "countsAsActiveLane": false,
  "pendingEffort": false
}
```
Stderr (verbatim):
```text

```

## C27

Argv (shell-quoted for display only):
```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/onboarding-probe-zrs28an7 task update 1 --go --team card-fixture --name lead
```
Exit: 0
Stdout (verbatim):
```text
updated task #1 -> pending
```
Stderr (verbatim):
```text

```
