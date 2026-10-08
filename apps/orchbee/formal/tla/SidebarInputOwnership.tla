-------------------------- MODULE SidebarInputOwnership --------------------------
EXTENDS TLC

(* One symbolic key/paste fragment can span arbitrarily ordered reads, owner
   retirement and successful frame publication. Tree identity, menu commands,
   terminal mode writes and host decoding have their own refinement evidence. *)
VARIABLES owner, menuReturn, packetOwned, paintPending, fragment, bornOwned, leaked
vars == <<owner, menuReturn, packetOwned, paintPending, fragment, bornOwned, leaked>>
Owners == {"terminal", "sidebar", "menu", "other"}
OwnsInput == owner # "terminal" \/ packetOwned \/ paintPending
CapturedFragment == IF fragment = "none" THEN "none" ELSE "captured"

Init ==
  /\ owner = "terminal" /\ menuReturn = "terminal"
  /\ packetOwned = FALSE /\ paintPending = FALSE
  /\ fragment = "none" /\ bornOwned = FALSE /\ leaked = FALSE

EnterSidebar ==
  /\ owner = "terminal" /\ ~packetOwned /\ ~paintPending
  /\ owner' = "sidebar" /\ packetOwned' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<menuReturn, paintPending, bornOwned, leaked>>

SidebarKey ==
  /\ owner = "sidebar" /\ ~packetOwned /\ ~paintPending
  /\ UNCHANGED vars

(* A validated keyboard adjustment or release changes local geometry, not
   the keyboard owner. Its packet and pending fragments wait for fresh paint. *)
ResizeSidebar ==
  /\ owner \in {"terminal", "sidebar"} /\ ~packetOwned /\ ~paintPending
  /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<owner, menuReturn, bornOwned, leaked>>

OpenMenu ==
  /\ owner \in {"terminal", "sidebar"} /\ ~packetOwned /\ ~paintPending
  /\ menuReturn' = owner /\ owner' = "menu" /\ packetOwned' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<paintPending, bornOwned, leaked>>

CloseMenu ==
  /\ owner = "menu"
  /\ owner' = menuReturn /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<menuReturn, bornOwned, leaked>>

(* Explicit exit/activation and a hidden effective surface share retirement.
   If a menu covers the tree, the menu keeps capture but loses its return owner. *)
RetireSidebar ==
  /\ owner = "sidebar" \/ (owner = "menu" /\ menuReturn = "sidebar")
  /\ owner' = IF owner = "sidebar" THEN "terminal" ELSE "menu"
  /\ menuReturn' = "terminal" /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<bornOwned, leaked>>

CoveringCapture ==
  /\ owner # "other"
  /\ owner' = "other" /\ menuReturn' = "terminal"
  /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<bornOwned, leaked>>

EndCoveringCapture ==
  /\ owner = "other"
  /\ owner' = "terminal" /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<menuReturn, bornOwned, leaked>>

PublishFrame ==
  /\ paintPending /\ paintPending' = FALSE
  /\ UNCHANGED <<owner, menuReturn, packetOwned, fragment, bornOwned, leaked>>

PacketBoundary ==
  /\ packetOwned /\ packetOwned' = paintPending
  /\ UNCHANGED <<owner, menuReturn, paintPending, fragment, bornOwned, leaked>>

BeginFragment ==
  /\ fragment = "none"
  /\ fragment' = IF packetOwned \/ paintPending THEN "captured" ELSE "raw"
  /\ bornOwned' = OwnsInput
  /\ UNCHANGED <<owner, menuReturn, packetOwned, paintPending, leaked>>

FinishFragment ==
  /\ fragment # "none"
  /\ leaked' = leaked \/ (fragment = "raw" /\ ~OwnsInput /\ bornOwned)
  /\ fragment' = "none" /\ bornOwned' = FALSE
  /\ UNCHANGED <<owner, menuReturn, packetOwned, paintPending>>

Next == EnterSidebar \/ SidebarKey \/ ResizeSidebar \/ OpenMenu \/ CloseMenu \/ RetireSidebar
        \/ CoveringCapture \/ EndCoveringCapture \/ PublishFrame \/ PacketBoundary
        \/ BeginFragment \/ FinishFragment

TypeOK ==
  /\ owner \in Owners /\ menuReturn \in {"terminal", "sidebar"}
  /\ packetOwned \in BOOLEAN /\ paintPending \in BOOLEAN
  /\ fragment \in {"none", "raw", "captured"}
  /\ bornOwned \in BOOLEAN /\ leaked \in BOOLEAN
NoInputLeak == ~leaked
NoStaleFragment == fragment = "none" => ~bornOwned
Spec == Init /\ [][Next]_vars
==================================================================================
