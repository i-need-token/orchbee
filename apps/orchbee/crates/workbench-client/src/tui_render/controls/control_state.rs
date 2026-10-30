//! Control-state vocabulary of the shared components: surface, focus, pointer, navigation, current,
//! checked, availability, and operation states, combined into one [`InteractionState`].

use crate::tui_theme::Role;

/// The base surface owned by a control's compositor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ControlSurface {
    /// Persistent application chrome.
    Chrome,
    /// The Tab strip, which stays distinct from the terminal canvas.
    Strip,
    /// An opaque overlay surface.
    Overlay,
    /// The terminal canvas, where pane frames carry their titles.
    Canvas,
}

impl ControlSurface {
    pub(super) fn role(self) -> Role {
        match self {
            Self::Chrome => Role::SurfaceChrome,
            Self::Strip => Role::SurfaceStrip,
            Self::Overlay => Role::SurfaceOverlay,
            Self::Canvas => Role::SurfaceCanvas,
        }
    }
}

/// Whether this component's declared keyboard owner owns contextual input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FocusState {
    /// Another component owns keyboard input.
    Unfocused,
    /// This component's declared keyboard owner owns input.
    Focused,
}

/// Ephemeral pointer presentation that never changes focus or a domain value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerPresentation {
    /// No admitted pointer is over or armed on the target.
    Rest,
    /// The admitted pointer is over the target.
    Hovered,
    /// A valid gesture is armed and remains inside the target.
    Pressed,
}

/// Reversible keyboard navigation before acceptance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NavigationState {
    /// This item is not the provisional keyboard item.
    None,
    /// This item is the provisional keyboard item.
    Provisional,
    /// This item opened the submenu that now holds navigation: it keeps the selection surface
    /// without the marker.
    Open,
}

/// Whether this component represents the persistent current context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CurrentState {
    /// The component is not current.
    NotCurrent,
    /// The component represents the current context.
    Current,
}

/// Whether the owning value model marks this component as checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckedState {
    /// The value is not checked.
    Unchecked,
    /// The value is checked at the owner's declared commit boundary.
    Checked,
}

/// Whether the owner currently admits activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Availability {
    /// Activation is admitted.
    Enabled,
    /// Activation is rejected; the feature owner retains the localized reason.
    Disabled,
}

/// Operation presentation after an admitted activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OperationState {
    /// No operation is in flight.
    Idle,
    /// The owner reports an in-flight operation.
    Pending,
}

/// Independent visual axes supplied by the semantic owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InteractionState {
    pub(super) focus: FocusState,
    pub(super) pointer: PointerPresentation,
    pub(super) navigation: NavigationState,
    pub(super) current: CurrentState,
    pub(super) checked: CheckedState,
    pub(super) availability: Availability,
    pub(super) operation: OperationState,
}

impl InteractionState {
    /// Constructs the complete state without compressing independent axes into `selected`.
    pub(crate) const fn new(
        focus: FocusState,
        pointer: PointerPresentation,
        navigation: NavigationState,
        current: CurrentState,
        checked: CheckedState,
        availability: Availability,
        operation: OperationState,
    ) -> Self {
        Self {
            focus,
            pointer,
            navigation,
            current,
            checked,
            availability,
            operation,
        }
    }

    /// Replaces only ephemeral pointer presentation after exact hit geometry is resolved.
    pub(crate) const fn with_pointer(mut self, pointer: PointerPresentation) -> Self {
        self.pointer = pointer;
        self
    }

    pub(super) fn pointer_surface(self) -> Option<Role> {
        if self.availability == Availability::Disabled {
            return None;
        }
        match self.pointer {
            PointerPresentation::Rest => None,
            PointerPresentation::Hovered => Some(Role::SurfaceHover),
            PointerPresentation::Pressed => Some(Role::SurfacePressed),
        }
    }

    pub(super) fn interaction_role(self) -> Option<Role> {
        if self.pointer == PointerPresentation::Pressed
            && let Some(role) = self.pointer_surface()
        {
            return Some(role);
        }
        if matches!(
            self.navigation,
            NavigationState::Provisional | NavigationState::Open
        ) {
            return Some(Role::SurfaceSelection);
        }
        if let Some(role) = self.pointer_surface() {
            return Some(role);
        }
        if self.current == CurrentState::Current {
            return Some(Role::SurfaceCurrent);
        }
        None
    }
}
