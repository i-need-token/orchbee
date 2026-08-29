---------------- MODULE WorkbenchDocumentPlacementMigration ----------------
EXTENDS TLC

\* Bounded model of the one-way Workbench shared-document v1-to-v2 migration.
\* A present v2 file is always authoritative, even when invalid. A valid v1
\* file is read only when v2 is missing, is projected to local placements, and
\* is never changed. Rename-before-confirmation is reconciled by loading v2.

DocumentStates == {"missing", "valid", "invalid"}
Outcomes == {"none", "missing", "available", "unavailable", "indeterminate"}
Actions == {
  "init", "load-v2", "load-missing", "reject-legacy", "prepare-migration",
  "pre-rename-failure", "rename-migration", "confirm-migration",
  "post-rename-failure", "reload-v2"
}

VARIABLES
  legacy,
  initialLegacy,
  current,
  prepared,
  preparedPlacement,
  outcome,
  lastAction

vars == <<legacy, initialLegacy, current, prepared, preparedPlacement, outcome,
  lastAction>>

Init ==
  /\ legacy \in DocumentStates
  /\ initialLegacy = legacy
  /\ current \in DocumentStates
  /\ prepared = FALSE
  /\ preparedPlacement = "none"
  /\ outcome = "none"
  /\ lastAction = "init"

LoadV2 ==
  /\ current \in {"valid", "invalid"}
  /\ outcome' = IF current = "valid" THEN "available" ELSE "unavailable"
  /\ prepared' = FALSE
  /\ preparedPlacement' = "none"
  /\ UNCHANGED <<legacy, initialLegacy, current>>
  /\ lastAction' = "load-v2"

LoadMissing ==
  /\ current = "missing"
  /\ legacy = "missing"
  /\ outcome' = "missing"
  /\ prepared' = FALSE
  /\ preparedPlacement' = "none"
  /\ UNCHANGED <<legacy, initialLegacy, current>>
  /\ lastAction' = "load-missing"

RejectLegacy ==
  /\ current = "missing"
  /\ legacy = "invalid"
  /\ outcome' = "unavailable"
  /\ prepared' = FALSE
  /\ preparedPlacement' = "none"
  /\ UNCHANGED <<legacy, initialLegacy, current>>
  /\ lastAction' = "reject-legacy"

PrepareMigration ==
  /\ current = "missing"
  /\ legacy = "valid"
  /\ ~prepared
  /\ prepared' = TRUE
  /\ preparedPlacement' = "local-only"
  /\ outcome' = "none"
  /\ UNCHANGED <<legacy, initialLegacy, current>>
  /\ lastAction' = "prepare-migration"

PreRenameFailure ==
  /\ current = "missing"
  /\ prepared
  /\ prepared' = FALSE
  /\ preparedPlacement' = "none"
  /\ outcome' = "unavailable"
  /\ UNCHANGED <<legacy, initialLegacy, current>>
  /\ lastAction' = "pre-rename-failure"

RenameMigration ==
  /\ current = "missing"
  /\ prepared
  /\ preparedPlacement = "local-only"
  /\ current' = "valid"
  /\ outcome' = "none"
  /\ UNCHANGED <<legacy, initialLegacy, prepared, preparedPlacement>>
  /\ lastAction' = "rename-migration"

ConfirmMigration ==
  /\ current = "valid"
  /\ prepared
  /\ preparedPlacement = "local-only"
  /\ prepared' = FALSE
  /\ preparedPlacement' = "none"
  /\ outcome' = "available"
  /\ UNCHANGED <<legacy, initialLegacy, current>>
  /\ lastAction' = "confirm-migration"

PostRenameFailure ==
  /\ current = "valid"
  /\ prepared
  /\ preparedPlacement = "local-only"
  /\ prepared' = FALSE
  /\ preparedPlacement' = "none"
  /\ outcome' = "indeterminate"
  /\ UNCHANGED <<legacy, initialLegacy, current>>
  /\ lastAction' = "post-rename-failure"

ReloadV2 ==
  /\ current = "valid"
  /\ outcome \in {"indeterminate", "unavailable"}
  /\ outcome' = "available"
  /\ prepared' = FALSE
  /\ preparedPlacement' = "none"
  /\ UNCHANGED <<legacy, initialLegacy, current>>
  /\ lastAction' = "reload-v2"

Next ==
  \/ LoadV2
  \/ LoadMissing
  \/ RejectLegacy
  \/ PrepareMigration
  \/ PreRenameFailure
  \/ RenameMigration
  \/ ConfirmMigration
  \/ PostRenameFailure
  \/ ReloadV2

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ legacy \in DocumentStates
  /\ initialLegacy \in DocumentStates
  /\ current \in DocumentStates
  /\ prepared \in BOOLEAN
  /\ preparedPlacement \in {"none", "local-only"}
  /\ outcome \in Outcomes
  /\ lastAction \in Actions

LegacyIsNeverChanged == legacy = initialLegacy

PreparedDocumentIsLocalOnly == prepared => preparedPlacement = "local-only"

PresentV2NeverFallsBack ==
  current # "missing" /\ lastAction \in {"load-v2", "reload-v2"} =>
    outcome = IF current = "valid" THEN "available" ELSE "unavailable"

MigrationCreatesOnlyValidV2 ==
  lastAction = "rename-migration" => current = "valid"

IndeterminateRenameIsAuthoritative ==
  outcome = "indeterminate" => current = "valid"

=============================================================================
