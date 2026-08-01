--------------------- MODULE TerminalHistoryInteraction ---------------------
EXTENDS Naturals, TLC

CONSTANTS Attachments, MaxRevision, MaxGeneration, MaxSequence, MaxExportChunks

NoSequence == MaxSequence + 1
NoRevision == MaxRevision + 1
NoGeneration == MaxGeneration + 1

VARIABLES
  revision,
  generation,
  attached,
  rowInFlight,
  rowSequence,
  nextSequence,
  pendingAnchor,
  pendingAnchorValid,
  pendingAnchorSequence,
  committedAnchor,
  committedAnchorValid,
  historyEventInFlight,
  historyDirty,
  selectionPresent,
  selectionValid,
  exportInFlight,
  exportChunkInFlight,
  exportRemaining,
  searchActive,
  searchRevision,
  searchGeneration,
  searchAccepted,
  promotionSafe

vars ==
  <<revision, generation, attached, rowInFlight, rowSequence, nextSequence,
    pendingAnchor, pendingAnchorValid, pendingAnchorSequence, committedAnchor,
    committedAnchorValid, historyEventInFlight, historyDirty,
    selectionPresent, selectionValid, exportInFlight, exportChunkInFlight,
    exportRemaining, searchActive, searchRevision, searchGeneration,
    searchAccepted, promotionSafe>>

Init ==
  /\ revision = 0
  /\ generation = 1
  /\ attached = [attachment \in Attachments |-> TRUE]
  /\ rowInFlight = [attachment \in Attachments |-> FALSE]
  /\ rowSequence = [attachment \in Attachments |-> NoSequence]
  /\ nextSequence = [attachment \in Attachments |-> 1]
  /\ pendingAnchor = [attachment \in Attachments |-> FALSE]
  /\ pendingAnchorValid = [attachment \in Attachments |-> FALSE]
  /\ pendingAnchorSequence = [attachment \in Attachments |-> NoSequence]
  /\ committedAnchor = [attachment \in Attachments |-> FALSE]
  /\ committedAnchorValid = [attachment \in Attachments |-> FALSE]
  /\ historyEventInFlight = [attachment \in Attachments |-> FALSE]
  /\ historyDirty = [attachment \in Attachments |-> FALSE]
  /\ selectionPresent = [attachment \in Attachments |-> FALSE]
  /\ selectionValid = [attachment \in Attachments |-> FALSE]
  /\ exportInFlight = [attachment \in Attachments |-> FALSE]
  /\ exportChunkInFlight = [attachment \in Attachments |-> FALSE]
  /\ exportRemaining = [attachment \in Attachments |-> 0]
  /\ searchActive = [attachment \in Attachments |-> FALSE]
  /\ searchRevision = [attachment \in Attachments |-> NoRevision]
  /\ searchGeneration = [attachment \in Attachments |-> NoGeneration]
  /\ searchAccepted = [attachment \in Attachments |-> FALSE]
  /\ promotionSafe = TRUE

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
  /\ searchAccepted' = [attachment \in Attachments |-> FALSE]
  /\ UNCHANGED <<generation, attached, rowInFlight, rowSequence,
                  nextSequence, pendingAnchor, pendingAnchorValid,
                  pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, selectionPresent, selectionValid,
                  exportInFlight, exportChunkInFlight, exportRemaining,
                  searchRevision, searchGeneration, promotionSafe>>

ResizeOrScreenSwitch ==
  /\ revision < MaxRevision
  /\ generation < MaxGeneration
  /\ revision' = revision + 1
  /\ generation' = generation + 1
  /\ rowInFlight' = [attachment \in Attachments |-> FALSE]
  /\ rowSequence' = [attachment \in Attachments |-> NoSequence]
  /\ pendingAnchor' = [attachment \in Attachments |-> FALSE]
  /\ pendingAnchorValid' = [attachment \in Attachments |-> FALSE]
  /\ pendingAnchorSequence' = [attachment \in Attachments |-> NoSequence]
  /\ committedAnchor' = [attachment \in Attachments |-> FALSE]
  /\ committedAnchorValid' = [attachment \in Attachments |-> FALSE]
  /\ historyEventInFlight' =
       [attachment \in Attachments |-> attached[attachment]]
  /\ historyDirty' = [attachment \in Attachments |-> FALSE]
  /\ selectionPresent' = [attachment \in Attachments |-> FALSE]
  /\ selectionValid' = [attachment \in Attachments |-> FALSE]
  /\ exportInFlight' = [attachment \in Attachments |-> FALSE]
  /\ exportChunkInFlight' = [attachment \in Attachments |-> FALSE]
  /\ exportRemaining' = [attachment \in Attachments |-> 0]
  /\ searchActive' = [attachment \in Attachments |-> FALSE]
  /\ searchRevision' = [attachment \in Attachments |-> NoRevision]
  /\ searchGeneration' = [attachment \in Attachments |-> NoGeneration]
  /\ searchAccepted' = [attachment \in Attachments |-> FALSE]
  /\ UNCHANGED <<attached, nextSequence, promotionSafe>>

RequestRows(attachment) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ ~rowInFlight[attachment]
  /\ nextSequence[attachment] <= MaxSequence
  /\ rowInFlight' = [rowInFlight EXCEPT ![attachment] = TRUE]
  /\ rowSequence' = [rowSequence EXCEPT ![attachment] = nextSequence[attachment]]
  /\ nextSequence' = [nextSequence EXCEPT ![attachment] = @ + 1]
  /\ UNCHANGED <<revision, generation, attached, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, exportInFlight,
                  exportChunkInFlight, exportRemaining, searchActive,
                  searchRevision, searchGeneration, searchAccepted,
                  promotionSafe>>

CompleteRows(attachment) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ rowInFlight[attachment]
  /\ rowInFlight' = [rowInFlight EXCEPT ![attachment] = FALSE]
  /\ pendingAnchor' = [pendingAnchor EXCEPT ![attachment] = TRUE]
  /\ pendingAnchorValid' = [pendingAnchorValid EXCEPT ![attachment] = TRUE]
  /\ pendingAnchorSequence' =
       [pendingAnchorSequence EXCEPT ![attachment] = rowSequence[attachment]]
  /\ rowSequence' = [rowSequence EXCEPT ![attachment] = NoSequence]
  /\ UNCHANGED <<revision, generation, attached, nextSequence,
                  committedAnchor, committedAnchorValid, historyEventInFlight,
                  historyDirty, selectionPresent, selectionValid,
                  exportInFlight, exportChunkInFlight, exportRemaining,
                  searchActive, searchRevision, searchGeneration,
                  searchAccepted, promotionSafe>>

CancelRows(attachment) ==
  /\ attachment \in Attachments
  /\ rowInFlight[attachment]
  /\ rowInFlight' = [rowInFlight EXCEPT ![attachment] = FALSE]
  /\ rowSequence' = [rowSequence EXCEPT ![attachment] = NoSequence]
  /\ UNCHANGED <<revision, generation, attached, nextSequence,
                  pendingAnchor, pendingAnchorValid, pendingAnchorSequence,
                  committedAnchor, committedAnchorValid,
                  historyEventInFlight, historyDirty, selectionPresent,
                  selectionValid, exportInFlight, exportChunkInFlight,
                  exportRemaining, searchActive, searchRevision,
                  searchGeneration, searchAccepted, promotionSafe>>

RowsApplied(attachment, sequence) ==
  /\ attachment \in Attachments
  /\ sequence \in 1..MaxSequence
  /\ attached[attachment]
  /\ pendingAnchor[attachment]
  /\ pendingAnchorValid[attachment]
  /\ pendingAnchorSequence[attachment] = sequence
  /\ committedAnchor' = [committedAnchor EXCEPT ![attachment] = TRUE]
  /\ committedAnchorValid' = [committedAnchorValid EXCEPT ![attachment] = TRUE]
  /\ pendingAnchor' = [pendingAnchor EXCEPT ![attachment] = FALSE]
  /\ pendingAnchorValid' = [pendingAnchorValid EXCEPT ![attachment] = FALSE]
  /\ pendingAnchorSequence' =
       [pendingAnchorSequence EXCEPT ![attachment] = NoSequence]
  /\ promotionSafe' = promotionSafe /\ pendingAnchorValid[attachment]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, nextSequence, historyEventInFlight,
                  historyDirty, selectionPresent, selectionValid,
                  exportInFlight, exportChunkInFlight, exportRemaining,
                  searchActive, searchRevision, searchGeneration,
                  searchAccepted>>

StaleRowsApplied(attachment, sequence) ==
  /\ attachment \in Attachments
  /\ sequence \in 1..MaxSequence
  /\ (~attached[attachment]
      \/ ~pendingAnchor[attachment]
      \/ ~pendingAnchorValid[attachment]
      \/ pendingAnchorSequence[attachment] # sequence)
  /\ UNCHANGED vars

Evict(attachment) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ pendingAnchorValid' = [pendingAnchorValid EXCEPT ![attachment] = FALSE]
  /\ committedAnchorValid' = [committedAnchorValid EXCEPT ![attachment] = FALSE]
  /\ selectionValid' = [selectionValid EXCEPT ![attachment] = FALSE]
  /\ exportInFlight' = [exportInFlight EXCEPT ![attachment] = FALSE]
  /\ exportChunkInFlight' = [exportChunkInFlight EXCEPT ![attachment] = FALSE]
  /\ exportRemaining' = [exportRemaining EXCEPT ![attachment] = 0]
  /\ searchActive' = [searchActive EXCEPT ![attachment] = FALSE]
  /\ searchAccepted' = [searchAccepted EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, nextSequence, pendingAnchor,
                  pendingAnchorSequence, committedAnchor,
                  historyEventInFlight, historyDirty, selectionPresent,
                  searchRevision, searchGeneration, promotionSafe>>

ObserveHistory(attachment) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ historyEventInFlight[attachment]
  /\ historyEventInFlight' =
       [historyEventInFlight EXCEPT ![attachment] = historyDirty[attachment]]
  /\ historyDirty' = [historyDirty EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, nextSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, selectionPresent, selectionValid,
                  exportInFlight, exportChunkInFlight, exportRemaining,
                  searchActive, searchRevision, searchGeneration,
                  searchAccepted, promotionSafe>>

SetSelection(attachment) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ selectionPresent' = [selectionPresent EXCEPT ![attachment] = TRUE]
  /\ selectionValid' = [selectionValid EXCEPT ![attachment] = TRUE]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, nextSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  exportInFlight, exportChunkInFlight, exportRemaining,
                  searchActive, searchRevision, searchGeneration,
                  searchAccepted, promotionSafe>>

StartExport(attachment) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ selectionPresent[attachment]
  /\ selectionValid[attachment]
  /\ ~exportInFlight[attachment]
  /\ exportInFlight' = [exportInFlight EXCEPT ![attachment] = TRUE]
  /\ exportRemaining' = [exportRemaining EXCEPT ![attachment] = MaxExportChunks]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, nextSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, exportChunkInFlight,
                  searchActive, searchRevision, searchGeneration,
                  searchAccepted, promotionSafe>>

EmitExportChunk(attachment) ==
  /\ attachment \in Attachments
  /\ exportInFlight[attachment]
  /\ ~exportChunkInFlight[attachment]
  /\ exportRemaining[attachment] > 0
  /\ exportChunkInFlight' = [exportChunkInFlight EXCEPT ![attachment] = TRUE]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, nextSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, exportInFlight,
                  exportRemaining, searchActive, searchRevision,
                  searchGeneration, searchAccepted, promotionSafe>>

ApplyExportChunk(attachment) ==
  /\ attachment \in Attachments
  /\ exportInFlight[attachment]
  /\ exportChunkInFlight[attachment]
  /\ exportChunkInFlight' = [exportChunkInFlight EXCEPT ![attachment] = FALSE]
  /\ exportRemaining' = [exportRemaining EXCEPT ![attachment] = @ - 1]
  /\ exportInFlight' =
       [exportInFlight EXCEPT ![attachment] = exportRemaining[attachment] > 1]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, nextSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, searchActive,
                  searchRevision, searchGeneration, searchAccepted,
                  promotionSafe>>

CancelExport(attachment) ==
  /\ attachment \in Attachments
  /\ exportInFlight[attachment]
  /\ exportInFlight' = [exportInFlight EXCEPT ![attachment] = FALSE]
  /\ exportChunkInFlight' = [exportChunkInFlight EXCEPT ![attachment] = FALSE]
  /\ exportRemaining' = [exportRemaining EXCEPT ![attachment] = 0]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, nextSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, searchActive,
                  searchRevision, searchGeneration, searchAccepted,
                  promotionSafe>>

BeginSearch(attachment) ==
  /\ attachment \in Attachments
  /\ attached[attachment]
  /\ searchActive' = [searchActive EXCEPT ![attachment] = TRUE]
  /\ searchRevision' = [searchRevision EXCEPT ![attachment] = revision]
  /\ searchGeneration' = [searchGeneration EXCEPT ![attachment] = generation]
  /\ searchAccepted' = [searchAccepted EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, nextSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, exportInFlight,
                  exportChunkInFlight, exportRemaining, promotionSafe>>

AcceptSearchPage(attachment) ==
  /\ attachment \in Attachments
  /\ searchActive[attachment]
  /\ searchRevision[attachment] = revision
  /\ searchGeneration[attachment] = generation
  /\ searchAccepted' = [searchAccepted EXCEPT ![attachment] = TRUE]
  /\ UNCHANGED <<revision, generation, attached, rowInFlight,
                  rowSequence, nextSequence, pendingAnchor,
                  pendingAnchorValid, pendingAnchorSequence, committedAnchor,
                  committedAnchorValid, historyEventInFlight, historyDirty,
                  selectionPresent, selectionValid, exportInFlight,
                  exportChunkInFlight, exportRemaining, searchActive,
                  searchRevision, searchGeneration, promotionSafe>>

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
  /\ committedAnchor' = [committedAnchor EXCEPT ![attachment] = FALSE]
  /\ committedAnchorValid' = [committedAnchorValid EXCEPT ![attachment] = FALSE]
  /\ historyEventInFlight' =
       [historyEventInFlight EXCEPT ![attachment] = FALSE]
  /\ historyDirty' = [historyDirty EXCEPT ![attachment] = FALSE]
  /\ selectionPresent' = [selectionPresent EXCEPT ![attachment] = FALSE]
  /\ selectionValid' = [selectionValid EXCEPT ![attachment] = FALSE]
  /\ exportInFlight' = [exportInFlight EXCEPT ![attachment] = FALSE]
  /\ exportChunkInFlight' = [exportChunkInFlight EXCEPT ![attachment] = FALSE]
  /\ exportRemaining' = [exportRemaining EXCEPT ![attachment] = 0]
  /\ searchActive' = [searchActive EXCEPT ![attachment] = FALSE]
  /\ searchRevision' = [searchRevision EXCEPT ![attachment] = NoRevision]
  /\ searchGeneration' = [searchGeneration EXCEPT ![attachment] = NoGeneration]
  /\ searchAccepted' = [searchAccepted EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, generation, nextSequence, promotionSafe>>

Next ==
  \/ Mutate
  \/ ResizeOrScreenSwitch
  \/ \E attachment \in Attachments:
       \/ RequestRows(attachment)
       \/ CompleteRows(attachment)
       \/ CancelRows(attachment)
       \/ Evict(attachment)
       \/ ObserveHistory(attachment)
       \/ SetSelection(attachment)
       \/ StartExport(attachment)
       \/ EmitExportChunk(attachment)
       \/ ApplyExportChunk(attachment)
       \/ CancelExport(attachment)
       \/ BeginSearch(attachment)
       \/ AcceptSearchPage(attachment)
       \/ Detach(attachment)
       \/ \E sequence \in 1..MaxSequence:
            \/ RowsApplied(attachment, sequence)
            \/ StaleRowsApplied(attachment, sequence)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ \A attachment \in Attachments: WF_vars(CompleteRows(attachment))
  /\ \A attachment \in Attachments: WF_vars(ObserveHistory(attachment))
  /\ \A attachment \in Attachments: WF_vars(EmitExportChunk(attachment))
  /\ \A attachment \in Attachments: WF_vars(ApplyExportChunk(attachment))

TypeOK ==
  /\ revision \in 0..MaxRevision
  /\ generation \in 1..MaxGeneration
  /\ attached \in [Attachments -> BOOLEAN]
  /\ rowInFlight \in [Attachments -> BOOLEAN]
  /\ rowSequence \in [Attachments -> 1..NoSequence]
  /\ nextSequence \in [Attachments -> 1..NoSequence]
  /\ pendingAnchor \in [Attachments -> BOOLEAN]
  /\ pendingAnchorValid \in [Attachments -> BOOLEAN]
  /\ pendingAnchorSequence \in [Attachments -> 1..NoSequence]
  /\ committedAnchor \in [Attachments -> BOOLEAN]
  /\ committedAnchorValid \in [Attachments -> BOOLEAN]
  /\ historyEventInFlight \in [Attachments -> BOOLEAN]
  /\ historyDirty \in [Attachments -> BOOLEAN]
  /\ selectionPresent \in [Attachments -> BOOLEAN]
  /\ selectionValid \in [Attachments -> BOOLEAN]
  /\ exportInFlight \in [Attachments -> BOOLEAN]
  /\ exportChunkInFlight \in [Attachments -> BOOLEAN]
  /\ exportRemaining \in [Attachments -> 0..MaxExportChunks]
  /\ searchActive \in [Attachments -> BOOLEAN]
  /\ searchRevision \in [Attachments -> 0..NoRevision]
  /\ searchGeneration \in [Attachments -> 1..NoGeneration]
  /\ searchAccepted \in [Attachments -> BOOLEAN]
  /\ promotionSafe \in BOOLEAN

OneBoundedInteractionFlight ==
  \A attachment \in Attachments:
    /\ rowInFlight[attachment] \in BOOLEAN
    /\ historyEventInFlight[attachment] \in BOOLEAN
    /\ exportChunkInFlight[attachment] \in BOOLEAN

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

SearchResultHasOneCurrentRevision ==
  \A attachment \in Attachments:
    searchAccepted[attachment] =>
      /\ searchActive[attachment]
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
      /\ ~searchActive[attachment]
      /\ ~searchAccepted[attachment]

RowFlightEventuallySettles ==
  \A attachment \in Attachments:
    rowInFlight[attachment] ~> ~rowInFlight[attachment]

HistoryEventEventuallyObserved ==
  \A attachment \in Attachments:
    historyEventInFlight[attachment] ~> ~historyEventInFlight[attachment]

ExportEventuallyCompletes ==
  \A attachment \in Attachments:
    exportInFlight[attachment] ~> ~exportInFlight[attachment]

=============================================================================
