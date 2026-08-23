------------------ MODULE DesktopTerminalMetadataDelivery ------------------
EXTENDS Naturals, TLC

MaxRevision == 3
Revisions == 0..MaxRevision
MetadataValues == {"none", "shell", "agent", "editor"}
NoRevision == MaxRevision + 1
NoValue == "none"

VARIABLES hostRevision, hostMetadataRevision, hostMetadata,
  preparedRevision, preparedMetadata, publishedRevision,
  desktopRevision, desktopMetadata, clientRevision, clientMetadata,
  tabVisible, closeActivity

vars == <<hostRevision, hostMetadataRevision, hostMetadata,
  preparedRevision, preparedMetadata, publishedRevision,
  desktopRevision, desktopMetadata, clientRevision, clientMetadata,
  tabVisible, closeActivity>>

Init ==
  /\ hostRevision = 0
  /\ hostMetadataRevision = 0
  /\ hostMetadata = NoValue
  /\ preparedRevision = NoRevision
  /\ preparedMetadata = NoValue
  /\ publishedRevision = 0
  /\ desktopRevision = 0
  /\ desktopMetadata = NoValue
  /\ clientRevision = 0
  /\ clientMetadata = NoValue
  /\ tabVisible = TRUE
  /\ closeActivity = "idle"

HostMetadataChange(value) ==
  /\ value \in MetadataValues \ {hostMetadata}
  /\ hostRevision < MaxRevision
  /\ hostRevision' = hostRevision + 1
  /\ hostMetadataRevision' = hostRevision + 1
  /\ hostMetadata' = value
  /\ UNCHANGED <<preparedRevision, preparedMetadata, publishedRevision,
       desktopRevision, desktopMetadata, clientRevision, clientMetadata,
       tabVisible, closeActivity>>

PrepareHostEvent ==
  /\ preparedRevision = NoRevision
  /\ publishedRevision < hostMetadataRevision
  /\ preparedRevision' = hostMetadataRevision
  /\ preparedMetadata' = hostMetadata
  /\ UNCHANGED <<hostRevision, hostMetadataRevision, hostMetadata,
       publishedRevision, desktopRevision, desktopMetadata, clientRevision,
       clientMetadata, tabVisible, closeActivity>>

CommitHostEvent ==
  /\ preparedRevision # NoRevision
  /\ publishedRevision < preparedRevision
  /\ publishedRevision' = preparedRevision
  /\ desktopRevision' = IF preparedRevision > desktopRevision
       THEN preparedRevision ELSE desktopRevision
  /\ desktopMetadata' = IF preparedRevision > desktopRevision
       THEN preparedMetadata ELSE desktopMetadata
  /\ preparedRevision' = NoRevision
  /\ preparedMetadata' = NoValue
  /\ UNCHANGED <<hostRevision, hostMetadataRevision, hostMetadata,
       clientRevision, clientMetadata, tabVisible, closeActivity>>

DiscardPreparedEvent ==
  /\ preparedRevision # NoRevision
  /\ preparedRevision' = NoRevision
  /\ preparedMetadata' = NoValue
  /\ UNCHANGED <<hostRevision, hostMetadataRevision, hostMetadata,
       publishedRevision, desktopRevision, desktopMetadata, clientRevision,
       clientMetadata, tabVisible, closeActivity>>

ClientObserveMetadata ==
  /\ desktopRevision > clientRevision
  /\ clientRevision' = desktopRevision
  /\ clientMetadata' = desktopMetadata
  /\ UNCHANGED <<hostRevision, hostMetadataRevision, hostMetadata,
       preparedRevision, preparedMetadata, publishedRevision,
       desktopRevision, desktopMetadata, tabVisible, closeActivity>>

ObserveStaleFrame ==
  /\ clientRevision > 0
  /\ UNCHANGED vars

SetTabVisibility(value) ==
  /\ value \in BOOLEAN
  /\ value # tabVisible
  /\ tabVisible' = value
  /\ UNCHANGED <<hostRevision, hostMetadataRevision, hostMetadata,
       preparedRevision, preparedMetadata, publishedRevision,
       desktopRevision, desktopMetadata, clientRevision, clientMetadata,
       closeActivity>>

Next ==
  \/ \E value \in MetadataValues: HostMetadataChange(value)
  \/ PrepareHostEvent
  \/ CommitHostEvent
  \/ DiscardPreparedEvent
  \/ ClientObserveMetadata
  \/ ObserveStaleFrame
  \/ \E value \in BOOLEAN: SetTabVisibility(value)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ hostRevision \in Revisions
  /\ hostMetadataRevision \in Revisions
  /\ hostMetadata \in MetadataValues
  /\ preparedRevision \in 0..NoRevision
  /\ preparedMetadata \in MetadataValues
  /\ publishedRevision \in Revisions
  /\ desktopRevision \in Revisions
  /\ desktopMetadata \in MetadataValues
  /\ clientRevision \in Revisions
  /\ clientMetadata \in MetadataValues
  /\ tabVisible \in BOOLEAN
  /\ closeActivity \in {"idle"}

RevisionOrder ==
  /\ publishedRevision <= hostMetadataRevision
  /\ desktopRevision <= publishedRevision
  /\ clientRevision <= desktopRevision

PreparedSnapshotIsCanonical ==
  preparedRevision # NoRevision =>
    /\ preparedRevision <= hostMetadataRevision
    /\ preparedRevision > publishedRevision

ClientMetadataIsRevisionTagged ==
  clientRevision = 0 => clientMetadata = NoValue

BackgroundDoesNotGateDelivery ==
  closeActivity = "idle"

=============================================================================
