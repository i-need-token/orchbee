------------------ MODULE DesktopTerminalMetadataDelivery ------------------
EXTENDS Naturals, TLC

TerminalIdentities == {"terminal-1", "terminal-2"}
NoIdentity == "no-terminal"
MaxRevision == 3
NoRevision == MaxRevision + 1
Revisions == 0..MaxRevision
MetadataValues == {"shell", "agent", "editor"}
NoValue == "none"

VARIABLES
  hostIdentity,
  retiredIdentities,
  hostRevision,
  hostMetadata,
  hostHistory,
  preparedIdentity,
  preparedRevision,
  preparedMetadata,
  publishedIdentity,
  publishedRevision,
  publishedMetadata,
  desktopIdentity,
  desktopRevision,
  desktopMetadata,
  clientIdentity,
  clientRevision,
  clientMetadata,
  tabVisible,
  staleFramesRejected,
  equalReplayObserved,
  equalConflictRejected

vars == <<
  hostIdentity,
  retiredIdentities,
  hostRevision,
  hostMetadata,
  hostHistory,
  preparedIdentity,
  preparedRevision,
  preparedMetadata,
  publishedIdentity,
  publishedRevision,
  publishedMetadata,
  desktopIdentity,
  desktopRevision,
  desktopMetadata,
  clientIdentity,
  clientRevision,
  clientMetadata,
  tabVisible,
  staleFramesRejected,
  equalReplayObserved,
  equalConflictRejected
>>

EmptyHistory ==
  [identity \in TerminalIdentities |->
    [revision \in Revisions |-> NoValue]]

Init ==
  /\ hostIdentity = "terminal-1"
  /\ retiredIdentities = {}
  /\ hostRevision = 0
  /\ hostMetadata = NoValue
  /\ hostHistory = EmptyHistory
  /\ preparedIdentity = NoIdentity
  /\ preparedRevision = NoRevision
  /\ preparedMetadata = NoValue
  /\ publishedIdentity = NoIdentity
  /\ publishedRevision = 0
  /\ publishedMetadata = NoValue
  /\ desktopIdentity = NoIdentity
  /\ desktopRevision = 0
  /\ desktopMetadata = NoValue
  /\ clientIdentity = NoIdentity
  /\ clientRevision = 0
  /\ clientMetadata = NoValue
  /\ tabVisible = TRUE
  /\ staleFramesRejected = 0
  /\ equalReplayObserved = FALSE
  /\ equalConflictRejected = FALSE

HostMetadataChange(value) ==
  /\ value \in MetadataValues
  /\ value # hostMetadata
  /\ hostRevision < MaxRevision
  /\ hostRevision' = hostRevision + 1
  /\ hostMetadata' = value
  /\ hostHistory' =
       [hostHistory EXCEPT ![hostIdentity][hostRevision + 1] = value]
  /\ UNCHANGED <<hostIdentity, retiredIdentities,
       preparedIdentity, preparedRevision, preparedMetadata,
       publishedIdentity, publishedRevision, publishedMetadata,
       desktopIdentity, desktopRevision, desktopMetadata,
       clientIdentity, clientRevision, clientMetadata, tabVisible,
       staleFramesRejected, equalReplayObserved, equalConflictRejected>>

PrepareHostEvent ==
  /\ preparedRevision = NoRevision
  /\ hostRevision > publishedRevision
  /\ preparedIdentity' = hostIdentity
  /\ preparedRevision' = hostRevision
  /\ preparedMetadata' = hostMetadata
  /\ UNCHANGED <<hostIdentity, retiredIdentities, hostRevision,
       hostMetadata, hostHistory, publishedIdentity, publishedRevision,
       publishedMetadata, desktopIdentity, desktopRevision, desktopMetadata,
       clientIdentity, clientRevision, clientMetadata, tabVisible,
       staleFramesRejected, equalReplayObserved, equalConflictRejected>>

CommitHostEvent ==
  /\ preparedRevision # NoRevision
  /\ publishedIdentity' = preparedIdentity
  /\ publishedRevision' = preparedRevision
  /\ publishedMetadata' = preparedMetadata
  /\ desktopIdentity' = preparedIdentity
  /\ desktopRevision' = preparedRevision
  /\ desktopMetadata' = preparedMetadata
  /\ preparedIdentity' = NoIdentity
  /\ preparedRevision' = NoRevision
  /\ preparedMetadata' = NoValue
  /\ UNCHANGED <<hostIdentity, retiredIdentities, hostRevision,
       hostMetadata, hostHistory, clientIdentity, clientRevision,
       clientMetadata, tabVisible, staleFramesRejected,
       equalReplayObserved, equalConflictRejected>>

DiscardPreparedEvent ==
  /\ preparedRevision # NoRevision
  /\ preparedIdentity' = NoIdentity
  /\ preparedRevision' = NoRevision
  /\ preparedMetadata' = NoValue
  /\ UNCHANGED <<hostIdentity, retiredIdentities, hostRevision,
       hostMetadata, hostHistory, publishedIdentity, publishedRevision,
       publishedMetadata, desktopIdentity, desktopRevision, desktopMetadata,
       clientIdentity, clientRevision, clientMetadata, tabVisible,
       staleFramesRejected, equalReplayObserved, equalConflictRejected>>

ClientObserveMetadata ==
  /\ desktopIdentity = hostIdentity
  /\ desktopRevision > clientRevision
  /\ clientIdentity' = desktopIdentity
  /\ clientRevision' = desktopRevision
  /\ clientMetadata' = desktopMetadata
  /\ UNCHANGED <<hostIdentity, retiredIdentities, hostRevision,
       hostMetadata, hostHistory, preparedIdentity, preparedRevision,
       preparedMetadata, publishedIdentity, publishedRevision,
       publishedMetadata, desktopIdentity, desktopRevision, desktopMetadata,
       tabVisible, staleFramesRejected, equalReplayObserved,
       equalConflictRejected>>

StartFreshTerminal(nextIdentity) ==
  /\ nextIdentity \in TerminalIdentities \ (retiredIdentities \cup {hostIdentity})
  /\ hostIdentity' = nextIdentity
  /\ retiredIdentities' = retiredIdentities \cup {hostIdentity}
  /\ hostRevision' = 0
  /\ hostMetadata' = NoValue
  /\ preparedIdentity' = NoIdentity
  /\ preparedRevision' = NoRevision
  /\ preparedMetadata' = NoValue
  /\ publishedIdentity' = NoIdentity
  /\ publishedRevision' = 0
  /\ publishedMetadata' = NoValue
  /\ desktopIdentity' = NoIdentity
  /\ desktopRevision' = 0
  /\ desktopMetadata' = NoValue
  /\ clientIdentity' = NoIdentity
  /\ clientRevision' = 0
  /\ clientMetadata' = NoValue
  /\ UNCHANGED <<hostHistory, tabVisible, staleFramesRejected,
       equalReplayObserved, equalConflictRejected>>

StaleFrame(identity, revision, metadata) ==
  /\ identity \in TerminalIdentities
  /\ revision \in 1..MaxRevision
  /\ metadata = hostHistory[identity][revision]
  /\ metadata # NoValue
  /\ \/ identity \in retiredIdentities
     \/ /\ identity = clientIdentity
        /\ revision < clientRevision

ObserveStaleFrame(identity, revision, metadata) ==
  /\ StaleFrame(identity, revision, metadata)
  /\ staleFramesRejected < 2
  /\ staleFramesRejected' = staleFramesRejected + 1
  /\ UNCHANGED <<hostIdentity, retiredIdentities, hostRevision,
       hostMetadata, hostHistory, preparedIdentity, preparedRevision,
       preparedMetadata, publishedIdentity, publishedRevision,
       publishedMetadata, desktopIdentity, desktopRevision, desktopMetadata,
       clientIdentity, clientRevision, clientMetadata, tabVisible,
       equalReplayObserved, equalConflictRejected>>

EqualReplayFrame(identity, revision, metadata) ==
  /\ identity = clientIdentity
  /\ revision = clientRevision
  /\ metadata = clientMetadata
  /\ revision > 0

ObserveEqualReplay(identity, revision, metadata) ==
  /\ EqualReplayFrame(identity, revision, metadata)
  /\ ~equalReplayObserved
  /\ equalReplayObserved' = TRUE
  /\ UNCHANGED <<hostIdentity, retiredIdentities, hostRevision,
       hostMetadata, hostHistory, preparedIdentity, preparedRevision,
       preparedMetadata, publishedIdentity, publishedRevision,
       publishedMetadata, desktopIdentity, desktopRevision, desktopMetadata,
       clientIdentity, clientRevision, clientMetadata, tabVisible,
       staleFramesRejected, equalConflictRejected>>

EqualConflictFrame(identity, revision, metadata) ==
  /\ identity = clientIdentity
  /\ revision = clientRevision
  /\ revision > 0
  /\ metadata \in MetadataValues \ {clientMetadata}

RejectEqualConflict(identity, revision, metadata) ==
  /\ EqualConflictFrame(identity, revision, metadata)
  /\ ~equalConflictRejected
  /\ equalConflictRejected' = TRUE
  /\ UNCHANGED <<hostIdentity, retiredIdentities, hostRevision,
       hostMetadata, hostHistory, preparedIdentity, preparedRevision,
       preparedMetadata, publishedIdentity, publishedRevision,
       publishedMetadata, desktopIdentity, desktopRevision, desktopMetadata,
       clientIdentity, clientRevision, clientMetadata, tabVisible,
       staleFramesRejected, equalReplayObserved>>

SetTabVisibility(value) ==
  /\ value \in BOOLEAN
  /\ value # tabVisible
  /\ tabVisible' = value
  /\ UNCHANGED <<hostIdentity, retiredIdentities, hostRevision,
       hostMetadata, hostHistory, preparedIdentity, preparedRevision,
       preparedMetadata, publishedIdentity, publishedRevision,
       publishedMetadata, desktopIdentity, desktopRevision, desktopMetadata,
       clientIdentity, clientRevision, clientMetadata, staleFramesRejected,
       equalReplayObserved, equalConflictRejected>>

Next ==
  \/ \E value \in MetadataValues: HostMetadataChange(value)
  \/ PrepareHostEvent
  \/ CommitHostEvent
  \/ DiscardPreparedEvent
  \/ ClientObserveMetadata
  \/ \E identity \in TerminalIdentities, revision \in 1..MaxRevision,
       metadata \in MetadataValues: ObserveStaleFrame(identity, revision, metadata)
  \/ \E identity \in TerminalIdentities, revision \in 1..MaxRevision,
       metadata \in MetadataValues: ObserveEqualReplay(identity, revision, metadata)
  \/ \E identity \in TerminalIdentities, revision \in 1..MaxRevision,
       metadata \in MetadataValues: RejectEqualConflict(identity, revision, metadata)
  \/ \E identity \in TerminalIdentities: StartFreshTerminal(identity)
  \/ \E value \in BOOLEAN: SetTabVisibility(value)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ hostIdentity \in TerminalIdentities
  /\ retiredIdentities \subseteq TerminalIdentities
  /\ hostIdentity \notin retiredIdentities
  /\ hostRevision \in Revisions
  /\ hostMetadata \in MetadataValues \cup {NoValue}
  /\ hostHistory \in
       [TerminalIdentities -> [Revisions -> MetadataValues \cup {NoValue}]]
  /\ preparedIdentity \in TerminalIdentities \cup {NoIdentity}
  /\ preparedRevision \in Revisions \cup {NoRevision}
  /\ preparedMetadata \in MetadataValues \cup {NoValue}
  /\ publishedIdentity \in TerminalIdentities \cup {NoIdentity}
  /\ publishedRevision \in Revisions
  /\ publishedMetadata \in MetadataValues \cup {NoValue}
  /\ desktopIdentity \in TerminalIdentities \cup {NoIdentity}
  /\ desktopRevision \in Revisions
  /\ desktopMetadata \in MetadataValues \cup {NoValue}
  /\ clientIdentity \in TerminalIdentities \cup {NoIdentity}
  /\ clientRevision \in Revisions
  /\ clientMetadata \in MetadataValues \cup {NoValue}
  /\ tabVisible \in BOOLEAN
  /\ staleFramesRejected \in 0..2
  /\ equalReplayObserved \in BOOLEAN
  /\ equalConflictRejected \in BOOLEAN

CanonicalSlot(identity, revision, metadata) ==
  \/ /\ identity = NoIdentity
     /\ revision = 0
     /\ metadata = NoValue
  \/ /\ identity \in TerminalIdentities
     /\ revision \in 1..MaxRevision
     /\ metadata = hostHistory[identity][revision]
     /\ metadata # NoValue

RevisionOrder ==
  /\ publishedRevision <= hostRevision
  /\ desktopRevision <= publishedRevision
  /\ clientRevision <= desktopRevision

HostMetadataIsRevisionTagged ==
  /\ hostHistory[hostIdentity][0] = NoValue
  /\ IF hostRevision = 0
       THEN hostMetadata = NoValue
       ELSE /\ hostMetadata = hostHistory[hostIdentity][hostRevision]
            /\ hostMetadata # NoValue

PreparedSnapshotIsCanonical ==
  IF preparedRevision = NoRevision
    THEN /\ preparedIdentity = NoIdentity
         /\ preparedMetadata = NoValue
    ELSE /\ preparedIdentity = hostIdentity
         /\ preparedRevision > publishedRevision
         /\ preparedRevision <= hostRevision
         /\ CanonicalSlot(preparedIdentity, preparedRevision, preparedMetadata)

DeliveredMetadataIsRevisionTagged ==
  /\ CanonicalSlot(publishedIdentity, publishedRevision, publishedMetadata)
  /\ CanonicalSlot(desktopIdentity, desktopRevision, desktopMetadata)
  /\ CanonicalSlot(clientIdentity, clientRevision, clientMetadata)

PipelineBelongsToCurrentIdentity ==
  /\ publishedIdentity \in {NoIdentity, hostIdentity}
  /\ desktopIdentity \in {NoIdentity, hostIdentity}
  /\ clientIdentity \in {NoIdentity, hostIdentity}

FreshTerminalStartsClear ==
  hostRevision = 0 =>
    /\ preparedRevision = NoRevision
    /\ publishedIdentity = NoIdentity
    /\ publishedRevision = 0
    /\ publishedMetadata = NoValue
    /\ desktopIdentity = NoIdentity
    /\ desktopRevision = 0
    /\ desktopMetadata = NoValue
    /\ clientIdentity = NoIdentity
    /\ clientRevision = 0
    /\ clientMetadata = NoValue

HiddenTabDoesNotGateDelivery ==
  /\ ~tabVisible
  /\ desktopIdentity = hostIdentity
  /\ desktopRevision > clientRevision
  => ENABLED ClientObserveMetadata

ClientRevisionStep ==
  clientIdentity' # clientIdentity \/ clientRevision' >= clientRevision

ClientPayloadStep ==
  /\ clientIdentity' = clientIdentity
  /\ clientRevision' = clientRevision
  => clientMetadata' = clientMetadata

ClientRevisionNeverRegressesWithinIdentity == [] [ClientRevisionStep]_vars

OnePayloadPerClientRevision == [] [ClientPayloadStep]_vars

=============================================================================
