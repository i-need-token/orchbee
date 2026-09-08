--------------------- MODULE TerminalHistoryInteraction ---------------------
EXTENDS Naturals, TLC

\* Refines runtime-model claims TH-002, TH-006, TH-010, TH-011, TH-012, and TH-016.
\* Current-view viewport row requests, history observation, selection, export
\* handoff, search, and detach each preserve one authoritative interaction
\* owner and one bounded acknowledgement-scoped flight, including the exact
\* export/chunk identity. Mutate advances the content revision without
\* cancelling a current-view row flight; exact-revision search is invalidated.
\* ResizeScreenOrScrollbackEviction abstracts
\* every generation-changing interaction invalidation, including application
\* clear and reset, so stale row, anchor, selection, export, and search work can
\* never promote after that mutation.

CONSTANTS Attachments, MaxRevision, MaxGeneration, MaxSequence, MaxExportChunks

NoSequence == MaxSequence + 1
NoChunkSequence == MaxExportChunks
NoRevision == MaxRevision + 1
NoGeneration == MaxGeneration + 1

VARIABLES
  revision,
  generation,
  attached,
  rowInFlight,
  rowSequence,
  lastRowSequence,
  pendingAnchor,
  pendingAnchorValid,
  pendingAnchorSequence,
  pendingAnchorRevision,
  pendingAnchorGeneration,
  committedAnchor,
  committedAnchorValid,
  committedAnchorSequence,
  committedAnchorRevision,
  committedAnchorGeneration,
  historyEventInFlight,
  historyDirty,
  selectionPresent,
  selectionValid,
  exportInFlight,
  exportChunkInFlight,
  exportRemaining,
  exportSequence,
  exportChunkSequence,
  lastExportSequence,
  searchActive,
  searchSequence,
  lastSearchSequence,
  searchRevision,
  searchGeneration,
  searchAccepted,
  promotionSafe,
  receivedInputSafe

coreVars ==
  <<revision, generation, attached, rowInFlight, rowSequence, lastRowSequence,
    pendingAnchor, pendingAnchorValid, pendingAnchorSequence, committedAnchor,
    pendingAnchorRevision, pendingAnchorGeneration, committedAnchorValid,
    committedAnchorSequence, committedAnchorRevision,
    committedAnchorGeneration, historyEventInFlight, historyDirty,
    selectionPresent, selectionValid, exportInFlight, exportChunkInFlight,
    exportRemaining, exportSequence, exportChunkSequence, lastExportSequence,
    searchActive, searchSequence, lastSearchSequence, searchRevision,
    searchGeneration,
    searchAccepted, promotionSafe>>

vars == <<coreVars, receivedInputSafe>>

Init ==
  /\ revision = 0
  /\ generation = 1
  /\ attached = [attachment \in Attachments |-> TRUE]
  /\ rowInFlight = [attachment \in Attachments |-> FALSE]
  /\ rowSequence = [attachment \in Attachments |-> NoSequence]
  /\ lastRowSequence = [attachment \in Attachments |-> 0]
  /\ pendingAnchor = [attachment \in Attachments |-> FALSE]
  /\ pendingAnchorValid = [attachment \in Attachments |-> FALSE]
  /\ pendingAnchorSequence = [attachment \in Attachments |-> NoSequence]
  /\ pendingAnchorRevision = [attachment \in Attachments |-> NoRevision]
  /\ pendingAnchorGeneration = [attachment \in Attachments |-> NoGeneration]
  /\ committedAnchor = [attachment \in Attachments |-> FALSE]
  /\ committedAnchorValid = [attachment \in Attachments |-> FALSE]
  /\ committedAnchorSequence = [attachment \in Attachments |-> NoSequence]
  /\ committedAnchorRevision = [attachment \in Attachments |-> NoRevision]
  /\ committedAnchorGeneration = [attachment \in Attachments |-> NoGeneration]
  /\ historyEventInFlight = [attachment \in Attachments |-> FALSE]
  /\ historyDirty = [attachment \in Attachments |-> FALSE]
  /\ selectionPresent = [attachment \in Attachments |-> FALSE]
  /\ selectionValid = [attachment \in Attachments |-> FALSE]
  /\ exportInFlight = [attachment \in Attachments |-> FALSE]
  /\ exportChunkInFlight = [attachment \in Attachments |-> FALSE]
  /\ exportRemaining = [attachment \in Attachments |-> 0]
  /\ exportSequence = [attachment \in Attachments |-> NoSequence]
  /\ exportChunkSequence = [attachment \in Attachments |-> NoChunkSequence]
  /\ lastExportSequence = [attachment \in Attachments |-> 0]
  /\ searchActive = [attachment \in Attachments |-> FALSE]
  /\ searchSequence = [attachment \in Attachments |-> NoSequence]
  /\ lastSearchSequence = [attachment \in Attachments |-> 0]
  /\ searchRevision = [attachment \in Attachments |-> NoRevision]
  /\ searchGeneration = [attachment \in Attachments |-> NoGeneration]
  /\ searchAccepted = [attachment \in Attachments |-> FALSE]
  /\ promotionSafe = TRUE
  /\ receivedInputSafe = TRUE

Mutate ==
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ historyEventInFlight' =
       [attachment \in Attachments |->
          IF attached[attachment] THEN TRUE
          ELSE historyEventInFlight[attachment]]
  /\ historyDirty' =
       [attachment \in Attachments |->
          IF attached[attachment] /\ historyEventInFlight[attachment] THEN TRUE
          ELSE historyDirty[attachment]]
  /\ searchActive' = [attachment \in Attachments |-> FALSE]
  /\ searchSequence' = [attachment \in Attachments |-> NoSequence]
  /\ searchAccepted' = [attachment \in Attachments |-> FALSE]
  /\ UNCHANGED <<generation, attached, rowInFlight, rowSequence,
                  lastRowSequence, pendingAnchor, pendingAnchorValid,
                  pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, selectionPresent, selectionValid,
                  exportInFlight, exportChunkInFlight, exportRemaining,
                  exportSequence, exportChunkSequence, lastExportSequence,
                  searchRevision, searchGeneration, promotionSafe,
                  pendingAnchorRevision, pendingAnchorGeneration,
                  committedAnchorSequence, committedAnchorRevision,
                  committedAnchorGeneration, lastSearchSequence, receivedInputSafe>>

ResizeScreenOrScrollbackEviction ==
  /\ revision < MaxRevision
  /\ generation < MaxGeneration
  /\ revision' = revision + 1
  /\ generation' = generation + 1
  /\ rowInFlight' = [attachment \in Attachments |-> FALSE]
  /\ rowSequence' = [attachment \in Attachments |-> NoSequence]
  /\ pendingAnchor' = [attachment \in Attachments |-> FALSE]
  /\ pendingAnchorValid' = [attachment \in Attachments |-> FALSE]
  /\ pendingAnchorSequence' = [attachment \in Attachments |-> NoSequence]
  /\ pendingAnchorRevision' = [attachment \in Attachments |-> NoRevision]
  /\ pendingAnchorGeneration' = [attachment \in Attachments |-> NoGeneration]
  /\ committedAnchor' = [attachment \in Attachments |-> FALSE]
  /\ committedAnchorValid' = [attachment \in Attachments |-> FALSE]
  /\ committedAnchorSequence' = [attachment \in Attachments |-> NoSequence]
  /\ committedAnchorRevision' = [attachment \in Attachments |-> NoRevision]
  /\ committedAnchorGeneration' = [attachment \in Attachments |-> NoGeneration]
  /\ historyEventInFlight' =
       [attachment \in Attachments |-> attached[attachment]]
  /\ historyDirty' = [attachment \in Attachments |-> FALSE]
  /\ selectionPresent' = [attachment \in Attachments |-> FALSE]
  /\ selectionValid' = [attachment \in Attachments |-> FALSE]
  /\ exportInFlight' = [attachment \in Attachments |-> FALSE]
  /\ exportChunkInFlight' = [attachment \in Attachments |-> FALSE]
  /\ exportRemaining' = [attachment \in Attachments |-> 0]
  /\ exportSequence' = [attachment \in Attachments |-> NoSequence]
  /\ exportChunkSequence' = [attachment \in Attachments |-> NoChunkSequence]
  /\ searchActive' = [attachment \in Attachments |-> FALSE]
  /\ searchSequence' = [attachment \in Attachments |-> NoSequence]
  /\ searchRevision' = [attachment \in Attachments |-> NoRevision]
  /\ searchGeneration' = [attachment \in Attachments |-> NoGeneration]
  /\ searchAccepted' = [attachment \in Attachments |-> FALSE]
  /\ UNCHANGED <<attached, lastRowSequence, lastExportSequence,
                  lastSearchSequence, promotionSafe, receivedInputSafe>>

RequestRows(attachment, sequence) ==
  /\ attachment \in Attachments
  /\ sequence \in 1..MaxSequence
  /\ attached[attachment]
  /\ ~rowInFlight[attachment]
  /\ sequence > lastRowSequence[attachment]
  /\ rowInFlight' = [rowInFlight EXCEPT ![attachment] = TRUE]
  /\ rowSequence' = [rowSequence EXCEPT ![attachment] = sequence]
  /\ lastRowSequence' = [lastRowSequence EXCEPT ![attachment] = sequence]
  /\ UNCHANGED <<revision, generation, attached, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, exportInFlight,
                  exportChunkInFlight, exportRemaining, exportSequence,
                  exportChunkSequence, lastExportSequence, searchActive,
                  searchRevision, searchGeneration, searchAccepted,
                  promotionSafe, pendingAnchorRevision, pendingAnchorGeneration, committedAnchorSequence, committedAnchorRevision, committedAnchorGeneration, searchSequence, lastSearchSequence, receivedInputSafe>>

CompleteRows(attachment) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ rowInFlight[attachment]
  /\ rowInFlight' = [rowInFlight EXCEPT ![attachment] = FALSE]
  /\ pendingAnchor' = [pendingAnchor EXCEPT ![attachment] = TRUE]
  /\ pendingAnchorValid' = [pendingAnchorValid EXCEPT ![attachment] = TRUE]
  /\ pendingAnchorSequence' =
       [pendingAnchorSequence EXCEPT ![attachment] = rowSequence[attachment]]
  /\ pendingAnchorRevision' =
       [pendingAnchorRevision EXCEPT ![attachment] = revision]
  /\ pendingAnchorGeneration' =
       [pendingAnchorGeneration EXCEPT ![attachment] = generation]
  /\ rowSequence' = [rowSequence EXCEPT ![attachment] = NoSequence]
  /\ UNCHANGED <<revision, generation, attached, lastRowSequence,
                  committedAnchor, committedAnchorValid, historyEventInFlight,
                  historyDirty, selectionPresent, selectionValid,
                  exportInFlight, exportChunkInFlight, exportRemaining,
                  exportSequence, exportChunkSequence, lastExportSequence,
                  searchActive, searchRevision, searchGeneration,
                  searchAccepted, promotionSafe, committedAnchorSequence,
                  committedAnchorRevision, committedAnchorGeneration,
                  searchSequence, lastSearchSequence, receivedInputSafe>>

CancelRows(attachment) ==
  /\ attachment \in Attachments
  /\ rowInFlight[attachment]
  /\ rowInFlight' = [rowInFlight EXCEPT ![attachment] = FALSE]
  /\ rowSequence' = [rowSequence EXCEPT ![attachment] = NoSequence]
  /\ UNCHANGED <<revision, generation, attached, lastRowSequence,
                  pendingAnchor, pendingAnchorValid, pendingAnchorSequence,
                  committedAnchor, committedAnchorValid,
                  historyEventInFlight, historyDirty, selectionPresent,
                  selectionValid, exportInFlight, exportChunkInFlight,
                  exportRemaining, exportSequence, exportChunkSequence,
                  lastExportSequence,
                  searchActive, searchRevision,
                  searchGeneration, searchAccepted, promotionSafe, pendingAnchorRevision, pendingAnchorGeneration, committedAnchorSequence, committedAnchorRevision, committedAnchorGeneration, searchSequence, lastSearchSequence, receivedInputSafe>>

ExactRowsAppliedInput(attachment, sequence, acknowledgedRevision, acknowledgedGeneration) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ pendingAnchor[attachment]
  /\ pendingAnchorValid[attachment]
  /\ pendingAnchorSequence[attachment] = sequence
  /\ pendingAnchorRevision[attachment] = acknowledgedRevision
  /\ pendingAnchorGeneration[attachment] = acknowledgedGeneration

MonitorRowsAppliedInput(attachment, sequence, acknowledgedRevision, acknowledgedGeneration) ==
  receivedInputSafe' =
    receivedInputSafe
      /\ (ExactRowsAppliedInput(attachment, sequence, acknowledgedRevision,
                                acknowledgedGeneration)
          \/ coreVars' = coreVars)

RowsApplied(attachment, sequence, acknowledgedRevision, acknowledgedGeneration) ==
  /\ attachment \in Attachments
  /\ sequence \in 1..MaxSequence
  /\ acknowledgedRevision \in 0..MaxRevision
  /\ acknowledgedGeneration \in 1..MaxGeneration
  /\ attached[attachment]
  /\ pendingAnchor[attachment]
  /\ pendingAnchorValid[attachment]
  /\ pendingAnchorSequence[attachment] = sequence
  /\ pendingAnchorRevision[attachment] = acknowledgedRevision
  /\ pendingAnchorGeneration[attachment] = acknowledgedGeneration
  /\ committedAnchor' = [committedAnchor EXCEPT ![attachment] = TRUE]
  /\ committedAnchorValid' = [committedAnchorValid EXCEPT ![attachment] = TRUE]
  /\ committedAnchorSequence' =
       [committedAnchorSequence EXCEPT ![attachment] = sequence]
  /\ committedAnchorRevision' =
       [committedAnchorRevision EXCEPT ![attachment] = acknowledgedRevision]
  /\ committedAnchorGeneration' =
       [committedAnchorGeneration EXCEPT ![attachment] = acknowledgedGeneration]
  /\ pendingAnchor' = [pendingAnchor EXCEPT ![attachment] = FALSE]
  /\ pendingAnchorValid' = [pendingAnchorValid EXCEPT ![attachment] = FALSE]
  /\ pendingAnchorSequence' =
       [pendingAnchorSequence EXCEPT ![attachment] = NoSequence]
  /\ pendingAnchorRevision' =
       [pendingAnchorRevision EXCEPT ![attachment] = NoRevision]
  /\ pendingAnchorGeneration' =
       [pendingAnchorGeneration EXCEPT ![attachment] = NoGeneration]
  /\ promotionSafe' = promotionSafe /\ pendingAnchorValid[attachment]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, lastRowSequence, historyEventInFlight,
                  historyDirty, selectionPresent, selectionValid,
                  exportInFlight, exportChunkInFlight, exportRemaining,
                  exportSequence, exportChunkSequence, lastExportSequence,
                  searchActive, searchRevision, searchGeneration,
                  searchAccepted, searchSequence, lastSearchSequence>>
  /\ MonitorRowsAppliedInput(attachment, sequence, acknowledgedRevision,
                             acknowledgedGeneration)

StaleRowsApplied(attachment, sequence, acknowledgedRevision, acknowledgedGeneration) ==
  /\ attachment \in Attachments
  /\ sequence \in 0..NoSequence
  /\ acknowledgedRevision \in 0..NoRevision
  /\ acknowledgedGeneration \in 1..NoGeneration
  /\ ~(attached[attachment]
       /\ pendingAnchor[attachment]
       /\ pendingAnchorValid[attachment]
       /\ pendingAnchorSequence[attachment] = sequence
       /\ pendingAnchorRevision[attachment] = acknowledgedRevision
       /\ pendingAnchorGeneration[attachment] = acknowledgedGeneration)
  /\ UNCHANGED coreVars
  /\ MonitorRowsAppliedInput(attachment, sequence, acknowledgedRevision,
                             acknowledgedGeneration)

Evict(attachment) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ pendingAnchorValid' = [pendingAnchorValid EXCEPT ![attachment] = FALSE]
  /\ committedAnchorValid' = [committedAnchorValid EXCEPT ![attachment] = FALSE]
  /\ selectionValid' = [selectionValid EXCEPT ![attachment] = FALSE]
  /\ exportInFlight' = [exportInFlight EXCEPT ![attachment] = FALSE]
  /\ exportChunkInFlight' = [exportChunkInFlight EXCEPT ![attachment] = FALSE]
  /\ exportRemaining' = [exportRemaining EXCEPT ![attachment] = 0]
  /\ exportSequence' = [exportSequence EXCEPT ![attachment] = NoSequence]
  /\ exportChunkSequence' =
       [exportChunkSequence EXCEPT ![attachment] = NoChunkSequence]
  /\ searchActive' = [searchActive EXCEPT ![attachment] = FALSE]
  /\ searchSequence' = [searchSequence EXCEPT ![attachment] = NoSequence]
  /\ searchAccepted' = [searchAccepted EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, lastRowSequence, pendingAnchor,
                  pendingAnchorSequence, committedAnchor,
                  historyEventInFlight, historyDirty, selectionPresent,
                  lastExportSequence, searchRevision, searchGeneration,
                  promotionSafe, pendingAnchorRevision,
                  pendingAnchorGeneration, committedAnchorSequence,
                  committedAnchorRevision, committedAnchorGeneration,
                  lastSearchSequence, receivedInputSafe>>

ObserveHistory(attachment) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ historyEventInFlight[attachment]
  /\ historyEventInFlight' =
       [historyEventInFlight EXCEPT ![attachment] = historyDirty[attachment]]
  /\ historyDirty' = [historyDirty EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, lastRowSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, selectionPresent, selectionValid,
                  exportInFlight, exportChunkInFlight, exportRemaining,
                  exportSequence, exportChunkSequence, lastExportSequence,
                  searchActive, searchRevision, searchGeneration,
                  searchAccepted, promotionSafe, pendingAnchorRevision, pendingAnchorGeneration, committedAnchorSequence, committedAnchorRevision, committedAnchorGeneration, searchSequence, lastSearchSequence, receivedInputSafe>>

SetSelection(attachment) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ selectionPresent' = [selectionPresent EXCEPT ![attachment] = TRUE]
  /\ selectionValid' = [selectionValid EXCEPT ![attachment] = TRUE]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, lastRowSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  exportInFlight, exportChunkInFlight, exportRemaining,
                  exportSequence, exportChunkSequence, lastExportSequence,
                  searchActive, searchRevision, searchGeneration,
                  searchAccepted, promotionSafe, pendingAnchorRevision, pendingAnchorGeneration, committedAnchorSequence, committedAnchorRevision, committedAnchorGeneration, searchSequence, lastSearchSequence, receivedInputSafe>>

StartExport(attachment, sequence) ==
  /\ attachment \in Attachments
  /\ sequence \in 1..MaxSequence
  /\ attached[attachment]
  /\ selectionPresent[attachment]
  /\ selectionValid[attachment]
  /\ ~exportInFlight[attachment]
  /\ sequence > lastExportSequence[attachment]
  /\ exportInFlight' = [exportInFlight EXCEPT ![attachment] = TRUE]
  /\ exportRemaining' = [exportRemaining EXCEPT ![attachment] = MaxExportChunks]
  /\ exportSequence' = [exportSequence EXCEPT ![attachment] = sequence]
  /\ exportChunkSequence' = [exportChunkSequence EXCEPT ![attachment] = 0]
  /\ lastExportSequence' = [lastExportSequence EXCEPT ![attachment] = sequence]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, lastRowSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, exportChunkInFlight,
                  searchActive, searchRevision, searchGeneration,
                  searchAccepted, promotionSafe, pendingAnchorRevision, pendingAnchorGeneration, committedAnchorSequence, committedAnchorRevision, committedAnchorGeneration, searchSequence, lastSearchSequence, receivedInputSafe>>

EmitExportChunk(attachment) ==
  /\ attachment \in Attachments
  /\ exportInFlight[attachment]
  /\ ~exportChunkInFlight[attachment]
  /\ exportRemaining[attachment] > 0
  /\ exportChunkInFlight' = [exportChunkInFlight EXCEPT ![attachment] = TRUE]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, lastRowSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, exportInFlight,
                  exportRemaining, exportSequence, exportChunkSequence,
                  lastExportSequence,
                  searchActive, searchRevision,
                  searchGeneration, searchAccepted, promotionSafe, pendingAnchorRevision, pendingAnchorGeneration, committedAnchorSequence, committedAnchorRevision, committedAnchorGeneration, searchSequence, lastSearchSequence, receivedInputSafe>>

ApplyExportChunk(attachment, sequence, chunkSequence) ==
  /\ attachment \in Attachments
  /\ sequence \in 1..MaxSequence
  /\ chunkSequence \in 0..(MaxExportChunks - 1)
  /\ exportInFlight[attachment]
  /\ exportChunkInFlight[attachment]
  /\ exportSequence[attachment] = sequence
  /\ exportChunkSequence[attachment] = chunkSequence
  /\ exportChunkInFlight' = [exportChunkInFlight EXCEPT ![attachment] = FALSE]
  /\ exportRemaining' = [exportRemaining EXCEPT ![attachment] = @ - 1]
  /\ exportInFlight' =
       [exportInFlight EXCEPT ![attachment] = exportRemaining[attachment] > 1]
  /\ exportSequence' =
       [exportSequence EXCEPT
          ![attachment] = IF exportRemaining[attachment] > 1 THEN @ ELSE NoSequence]
  /\ exportChunkSequence' =
       [exportChunkSequence EXCEPT
          ![attachment] = IF exportRemaining[attachment] > 1
            THEN @ + 1
            ELSE NoChunkSequence]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, lastRowSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, lastExportSequence,
                  searchActive,
                  searchRevision, searchGeneration, searchAccepted,
                  promotionSafe, pendingAnchorRevision, pendingAnchorGeneration, committedAnchorSequence, committedAnchorRevision, committedAnchorGeneration, searchSequence, lastSearchSequence, receivedInputSafe>>

StaleExportChunkApplied(attachment, sequence, chunkSequence) ==
  /\ attachment \in Attachments
  /\ sequence \in 1..MaxSequence
  /\ chunkSequence \in 0..(MaxExportChunks - 1)
  /\ (~exportInFlight[attachment]
      \/ ~exportChunkInFlight[attachment]
      \/ exportSequence[attachment] # sequence
      \/ exportChunkSequence[attachment] # chunkSequence)
  /\ UNCHANGED vars

ApplyCurrentExportChunk(attachment) ==
  ApplyExportChunk(
    attachment,
    exportSequence[attachment],
    exportChunkSequence[attachment]
  )

CancelExport(attachment) ==
  /\ attachment \in Attachments
  /\ exportInFlight[attachment]
  /\ exportInFlight' = [exportInFlight EXCEPT ![attachment] = FALSE]
  /\ exportChunkInFlight' = [exportChunkInFlight EXCEPT ![attachment] = FALSE]
  /\ exportRemaining' = [exportRemaining EXCEPT ![attachment] = 0]
  /\ exportSequence' = [exportSequence EXCEPT ![attachment] = NoSequence]
  /\ exportChunkSequence' =
       [exportChunkSequence EXCEPT ![attachment] = NoChunkSequence]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, lastRowSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, lastExportSequence,
                  searchActive,
                  searchRevision, searchGeneration, searchAccepted,
                  promotionSafe, pendingAnchorRevision, pendingAnchorGeneration, committedAnchorSequence, committedAnchorRevision, committedAnchorGeneration, searchSequence, lastSearchSequence, receivedInputSafe>>

BeginSearch(attachment, sequence) ==
  /\ attachment \in Attachments
  /\ sequence \in 1..MaxSequence
  /\ attached[attachment]
  /\ sequence > lastSearchSequence[attachment]
  /\ searchActive' = [searchActive EXCEPT ![attachment] = TRUE]
  /\ searchSequence' = [searchSequence EXCEPT ![attachment] = sequence]
  /\ lastSearchSequence' =
       [lastSearchSequence EXCEPT ![attachment] = sequence]
  /\ searchRevision' = [searchRevision EXCEPT ![attachment] = revision]
  /\ searchGeneration' = [searchGeneration EXCEPT ![attachment] = generation]
  /\ searchAccepted' = [searchAccepted EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, lastRowSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, exportInFlight,
                  exportChunkInFlight, exportRemaining, exportSequence,
                  exportChunkSequence, lastExportSequence, promotionSafe,
                  pendingAnchorRevision, pendingAnchorGeneration,
                  committedAnchorSequence, committedAnchorRevision,
                  committedAnchorGeneration, receivedInputSafe>>

ExactSearchPageInput(attachment, sequence, requestRevision, requestGeneration) ==
  /\ attachment \in Attachments
  /\ searchActive[attachment]
  /\ searchSequence[attachment] = sequence
  /\ searchRevision[attachment] = requestRevision
  /\ searchGeneration[attachment] = requestGeneration
  /\ requestRevision = revision
  /\ requestGeneration = generation

MonitorSearchPageInput(attachment, sequence, requestRevision, requestGeneration) ==
  receivedInputSafe' =
    receivedInputSafe
      /\ (ExactSearchPageInput(attachment, sequence, requestRevision,
                               requestGeneration)
          \/ coreVars' = coreVars)

AcceptSearchPage(attachment, sequence, requestRevision, requestGeneration) ==
  /\ attachment \in Attachments
  /\ sequence \in 1..MaxSequence
  /\ requestRevision \in 0..MaxRevision
  /\ requestGeneration \in 1..MaxGeneration
  /\ searchActive[attachment]
  /\ searchSequence[attachment] = sequence
  /\ searchRevision[attachment] = requestRevision
  /\ searchGeneration[attachment] = requestGeneration
  /\ requestRevision = revision
  /\ requestGeneration = generation
  /\ searchAccepted' = [searchAccepted EXCEPT ![attachment] = TRUE]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, lastRowSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, exportInFlight,
                  exportChunkInFlight, exportRemaining, exportSequence,
                  exportChunkSequence, lastExportSequence, searchActive,
                  searchRevision, searchGeneration, promotionSafe,
                  pendingAnchorRevision, pendingAnchorGeneration,
                  committedAnchorSequence, committedAnchorRevision,
                  committedAnchorGeneration, searchSequence,
                  lastSearchSequence>>
  /\ MonitorSearchPageInput(attachment, sequence, requestRevision,
                            requestGeneration)

StaleSearchPage(attachment, sequence, requestRevision, requestGeneration) ==
  /\ attachment \in Attachments
  /\ sequence \in 0..NoSequence
  /\ requestRevision \in 0..NoRevision
  /\ requestGeneration \in 1..NoGeneration
  /\ ~(searchActive[attachment]
       /\ searchSequence[attachment] = sequence
       /\ searchRevision[attachment] = requestRevision
       /\ searchGeneration[attachment] = requestGeneration
       /\ requestRevision = revision
       /\ requestGeneration = generation)
  /\ UNCHANGED coreVars
  /\ MonitorSearchPageInput(attachment, sequence, requestRevision,
                            requestGeneration)

Detach(attachment) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ attached' = [attached EXCEPT ![attachment] = FALSE]
  /\ rowInFlight' = [rowInFlight EXCEPT ![attachment] = FALSE]
  /\ rowSequence' = [rowSequence EXCEPT ![attachment] = NoSequence]
  /\ pendingAnchor' = [pendingAnchor EXCEPT ![attachment] = FALSE]
  /\ pendingAnchorValid' = [pendingAnchorValid EXCEPT ![attachment] = FALSE]
  /\ pendingAnchorSequence' =
       [pendingAnchorSequence EXCEPT ![attachment] = NoSequence]
  /\ pendingAnchorRevision' =
       [pendingAnchorRevision EXCEPT ![attachment] = NoRevision]
  /\ pendingAnchorGeneration' =
       [pendingAnchorGeneration EXCEPT ![attachment] = NoGeneration]
  /\ committedAnchor' = [committedAnchor EXCEPT ![attachment] = FALSE]
  /\ committedAnchorValid' = [committedAnchorValid EXCEPT ![attachment] = FALSE]
  /\ committedAnchorSequence' =
       [committedAnchorSequence EXCEPT ![attachment] = NoSequence]
  /\ committedAnchorRevision' =
       [committedAnchorRevision EXCEPT ![attachment] = NoRevision]
  /\ committedAnchorGeneration' =
       [committedAnchorGeneration EXCEPT ![attachment] = NoGeneration]
  /\ historyEventInFlight' =
       [historyEventInFlight EXCEPT ![attachment] = FALSE]
  /\ historyDirty' = [historyDirty EXCEPT ![attachment] = FALSE]
  /\ selectionPresent' = [selectionPresent EXCEPT ![attachment] = FALSE]
  /\ selectionValid' = [selectionValid EXCEPT ![attachment] = FALSE]
  /\ exportInFlight' = [exportInFlight EXCEPT ![attachment] = FALSE]
  /\ exportChunkInFlight' = [exportChunkInFlight EXCEPT ![attachment] = FALSE]
  /\ exportRemaining' = [exportRemaining EXCEPT ![attachment] = 0]
  /\ exportSequence' = [exportSequence EXCEPT ![attachment] = NoSequence]
  /\ exportChunkSequence' =
       [exportChunkSequence EXCEPT ![attachment] = NoChunkSequence]
  /\ searchActive' = [searchActive EXCEPT ![attachment] = FALSE]
  /\ searchSequence' = [searchSequence EXCEPT ![attachment] = NoSequence]
  /\ searchRevision' = [searchRevision EXCEPT ![attachment] = NoRevision]
  /\ searchGeneration' = [searchGeneration EXCEPT ![attachment] = NoGeneration]
  /\ searchAccepted' = [searchAccepted EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, generation, lastRowSequence, lastExportSequence,
                  lastSearchSequence, promotionSafe, receivedInputSafe>>

Next ==
  \/ Mutate
  \/ ResizeScreenOrScrollbackEviction
  \/ \E attachment \in Attachments:
       \/ CompleteRows(attachment)
       \/ CancelRows(attachment)
       \/ Evict(attachment)
       \/ ObserveHistory(attachment)
       \/ SetSelection(attachment)
       \/ EmitExportChunk(attachment)
       \/ CancelExport(attachment)
       \/ Detach(attachment)
       \/ \E sequence \in 1..MaxSequence:
             \/ RequestRows(attachment, sequence)
             \/ StartExport(attachment, sequence)
             \/ BeginSearch(attachment, sequence)
             \/ \E chunkSequence \in 0..(MaxExportChunks - 1):
                  \/ ApplyExportChunk(attachment, sequence, chunkSequence)
                  \/ StaleExportChunkApplied(attachment, sequence, chunkSequence)
       \/ \E sequence \in 0..NoSequence,
             acknowledgedRevision \in 0..NoRevision,
             acknowledgedGeneration \in 1..NoGeneration:
            \/ RowsApplied(attachment, sequence, acknowledgedRevision,
                           acknowledgedGeneration)
            \/ StaleRowsApplied(attachment, sequence, acknowledgedRevision,
                                acknowledgedGeneration)
       \/ \E sequence \in 0..NoSequence,
             requestRevision \in 0..NoRevision,
             requestGeneration \in 1..NoGeneration:
            \/ AcceptSearchPage(attachment, sequence, requestRevision,
                                requestGeneration)
            \/ StaleSearchPage(attachment, sequence, requestRevision,
                               requestGeneration)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ \A attachment \in Attachments: WF_vars(CompleteRows(attachment))
  /\ \A attachment \in Attachments: WF_vars(EmitExportChunk(attachment))

TypeOK ==
  /\ revision \in 0..MaxRevision
  /\ generation \in 1..MaxGeneration
  /\ attached \in [Attachments -> BOOLEAN]
  /\ rowInFlight \in [Attachments -> BOOLEAN]
  /\ rowSequence \in [Attachments -> 1..NoSequence]
  /\ lastRowSequence \in [Attachments -> 0..MaxSequence]
  /\ pendingAnchor \in [Attachments -> BOOLEAN]
  /\ pendingAnchorValid \in [Attachments -> BOOLEAN]
  /\ pendingAnchorSequence \in [Attachments -> 1..NoSequence]
  /\ pendingAnchorRevision \in [Attachments -> 0..NoRevision]
  /\ pendingAnchorGeneration \in [Attachments -> 1..NoGeneration]
  /\ committedAnchor \in [Attachments -> BOOLEAN]
  /\ committedAnchorValid \in [Attachments -> BOOLEAN]
  /\ committedAnchorSequence \in [Attachments -> 1..NoSequence]
  /\ committedAnchorRevision \in [Attachments -> 0..NoRevision]
  /\ committedAnchorGeneration \in [Attachments -> 1..NoGeneration]
  /\ historyEventInFlight \in [Attachments -> BOOLEAN]
  /\ historyDirty \in [Attachments -> BOOLEAN]
  /\ selectionPresent \in [Attachments -> BOOLEAN]
  /\ selectionValid \in [Attachments -> BOOLEAN]
  /\ exportInFlight \in [Attachments -> BOOLEAN]
  /\ exportChunkInFlight \in [Attachments -> BOOLEAN]
  /\ exportRemaining \in [Attachments -> 0..MaxExportChunks]
  /\ exportSequence \in [Attachments -> 1..NoSequence]
  /\ exportChunkSequence \in [Attachments -> 0..NoChunkSequence]
  /\ lastExportSequence \in [Attachments -> 0..MaxSequence]
  /\ searchActive \in [Attachments -> BOOLEAN]
  /\ searchSequence \in [Attachments -> 1..NoSequence]
  /\ lastSearchSequence \in [Attachments -> 0..MaxSequence]
  /\ searchRevision \in [Attachments -> 0..NoRevision]
  /\ searchGeneration \in [Attachments -> 1..NoGeneration]
  /\ searchAccepted \in [Attachments -> BOOLEAN]
  /\ promotionSafe \in BOOLEAN
  /\ receivedInputSafe \in BOOLEAN

OnlyValidPendingAnchorPromotes == promotionSafe

InvalidAnchorCannotNameContent ==
  \A attachment \in Attachments:
    /\ committedAnchorValid[attachment] => committedAnchor[attachment]
    /\ pendingAnchorValid[attachment] => pendingAnchor[attachment]

ExportRequiresCurrentSelection ==
  \A attachment \in Attachments:
    exportInFlight[attachment] =>
      /\ attached[attachment]
      /\ selectionPresent[attachment]
      /\ selectionValid[attachment]
      /\ exportRemaining[attachment] > 0
      /\ exportSequence[attachment] = lastExportSequence[attachment]
      /\ exportChunkSequence[attachment] =
           MaxExportChunks - exportRemaining[attachment]

ExportChunkFlightHasCurrentIdentity ==
  \A attachment \in Attachments:
    exportChunkInFlight[attachment] =>
      /\ exportInFlight[attachment]
      /\ exportChunkSequence[attachment] < MaxExportChunks

InactiveExportHasNoSequence ==
  \A attachment \in Attachments:
    ~exportInFlight[attachment] =>
      /\ exportSequence[attachment] = NoSequence
      /\ exportChunkSequence[attachment] = NoChunkSequence

SearchResultHasOneCurrentRevision ==
  \A attachment \in Attachments:
    searchAccepted[attachment] =>
      /\ searchActive[attachment]
      /\ searchSequence[attachment] = lastSearchSequence[attachment]
      /\ searchRevision[attachment] = revision
      /\ searchGeneration[attachment] = generation

DetachedAttachmentOwnsNothing ==
  \A attachment \in Attachments:
    ~attached[attachment] =>
      /\ ~rowInFlight[attachment]
      /\ ~pendingAnchor[attachment]
      /\ ~committedAnchor[attachment]
      /\ ~historyEventInFlight[attachment]
      /\ ~historyDirty[attachment]
      /\ ~selectionPresent[attachment]
      /\ ~exportInFlight[attachment]
      /\ ~exportChunkInFlight[attachment]
      /\ exportSequence[attachment] = NoSequence
      /\ exportChunkSequence[attachment] = NoChunkSequence
      /\ ~searchActive[attachment]
      /\ searchSequence[attachment] = NoSequence
      /\ ~searchAccepted[attachment]

PendingAnchorNamesExactRowIdentity ==
  \A attachment \in Attachments:
    pendingAnchorValid[attachment] =>
      /\ pendingAnchor[attachment]
      /\ pendingAnchorSequence[attachment] \in 1..MaxSequence
      /\ pendingAnchorRevision[attachment] \in 0..MaxRevision
      /\ pendingAnchorGeneration[attachment] \in 1..MaxGeneration

RejectedInputPreservation == receivedInputSafe

RowFlightEventuallySettles ==
  \A attachment \in Attachments:
    rowInFlight[attachment] ~> ~rowInFlight[attachment]

=============================================================================
