mod mutations;
mod real_locations;

use std::io;

use orchbee_files_protocol::{
    ConnectionId, ErrorCode, FilePath, FilesError, GrantSource, Operation, Root, ScopeUpdate,
    Token, WorkspaceId, WorkspaceScope,
};

use super::{RootFact, Scopes, View, path::NativePath};

fn workspace() -> WorkspaceId {
    WorkspaceId::from_bytes([1; 16]).unwrap()
}

fn connection(value: u64) -> ConnectionId {
    ConnectionId::new(value).unwrap()
}

/// The native spelling of the absolute path with these segments.
fn native(segments: &[&str]) -> String {
    let separator = std::path::MAIN_SEPARATOR_STR;
    let prefix = if cfg!(windows) { r"C:\" } else { "/" };
    format!("{prefix}{}", segments.join(separator))
}

fn file(segments: &[&str]) -> FilePath {
    FilePath::new(native(segments)).unwrap()
}

fn state(scope: WorkspaceScope) -> ScopeUpdate {
    ScopeUpdate {
        workspace: workspace(),
        scope,
    }
}

fn code<T>(result: Result<T, FilesError>) -> Option<ErrorCode> {
    result.err().map(|error| error.code)
}

fn view(scopes: &Scopes, reader: ConnectionId) -> View {
    scopes.view(workspace(), reader, Operation::FsStat).unwrap()
}

fn grant(scopes: &Scopes, reader: ConnectionId, path: &FilePath, source: GrantSource) {
    let view = view(scopes, reader);
    let admitted = view.grantable(path, source).unwrap();
    scopes.grant(&view, admitted, || false).unwrap().unwrap();
}

#[test]
fn a_view_keeps_its_scope_and_opened_files_outlive_the_close() {
    let scopes = Scopes::new();
    let reader = connection(1);
    assert_eq!(scopes.generation(workspace()), 0);
    assert_eq!(
        code(scopes.view(workspace(), reader, Operation::FsList)),
        Some(ErrorCode::Denied)
    );
    assert!(scopes.update(state(WorkspaceScope::Root(native(&["w", "project"])))));
    // A root restated as it was is no change.
    assert!(!scopes.update(state(WorkspaceScope::Root(native(&["w", "project"])))));
    assert_eq!(scopes.generation(workspace()), 1);

    let open = view(&scopes, reader);
    assert_eq!(code(open.read(&file(&["w", "project", "src"]))), None);
    assert_eq!(
        code(open.read(&file(&["w", "other"]))),
        Some(ErrorCode::OutsideScope)
    );
    let outside = file(&["etc", "hosts"]);
    assert_eq!(
        code(open.grantable(&outside, GrantSource::Tree)),
        Some(ErrorCode::OutsideScope)
    );
    grant(&scopes, reader, &outside, GrantSource::TerminalLink);
    // A view keeps the scope it started under.
    assert_eq!(code(open.read(&outside)), Some(ErrorCode::OutsideScope));
    assert_eq!(code(view(&scopes, reader).read(&outside)), None);
    // Another connection's opened files are not in its scope.
    assert_eq!(
        code(view(&scopes, connection(2)).read(&outside)),
        Some(ErrorCode::OutsideScope)
    );

    assert!(scopes.update(state(WorkspaceScope::Closed)));
    assert_eq!(
        code(scopes.view(workspace(), reader, Operation::FsGrant)),
        Some(ErrorCode::Denied)
    );
    let closed = view(&scopes, reader);
    assert_eq!(code(closed.read(&outside)), None);
    assert_eq!(
        code(closed.read(&file(&["w", "project", "src"]))),
        Some(ErrorCode::OutsideScope)
    );
    scopes.connection_lost(reader);
    assert_eq!(
        code(view(&scopes, reader).read(&outside)),
        Some(ErrorCode::OutsideScope)
    );
}

#[test]
fn a_file_stays_opened_until_each_grant_is_released() {
    let scopes = Scopes::new();
    scopes.update(state(WorkspaceScope::NoRoot));
    let reader = connection(1);
    let opened = file(&["notes", "a.md"]);
    grant(&scopes, reader, &opened, GrantSource::Restored);
    grant(&scopes, reader, &opened, GrantSource::OsOpen);
    // A grant whose call was cancelled records nothing.
    let cancelled = NativePath::parse(&native(&["notes", "b.md"])).unwrap();
    assert_eq!(
        scopes.grant(&view(&scopes, reader), cancelled, || true),
        None
    );
    assert_eq!(
        code(view(&scopes, reader).read(&file(&["notes", "b.md"]))),
        Some(ErrorCode::OutsideScope)
    );
    scopes.ungrant(&view(&scopes, reader), std::slice::from_ref(&opened));
    assert_eq!(code(view(&scopes, reader).read(&opened)), None);
    scopes.ungrant(
        &view(&scopes, reader),
        &[opened.clone(), file(&["never", "granted"])],
    );
    assert_eq!(
        code(view(&scopes, reader).read(&opened)),
        Some(ErrorCode::OutsideScope)
    );

    let full = view(&scopes, reader);
    for index in 0..super::MAXIMUM_OPENED {
        let path = NativePath::parse(&native(&["many", &index.to_string()])).unwrap();
        scopes.grant(&full, path, || false).unwrap().unwrap();
    }
    let one_more = NativePath::parse(&native(&["many", "one-more"])).unwrap();
    let refused = scopes
        .grant(&full, one_more, || false)
        .unwrap()
        .unwrap_err();
    assert_eq!(
        (refused.code, refused.reason),
        (
            ErrorCode::Unavailable,
            Some(Token::new("too-many-grants").unwrap())
        )
    );
    // Another grant of a file already opened is not one more file.
    let again = NativePath::parse(&native(&["many", "0"])).unwrap();
    scopes.grant(&full, again, || false).unwrap().unwrap();
}

#[test]
fn the_root_fact_reads_the_real_location_which_then_admits_its_spelling() {
    let scopes = Scopes::new();
    let reported = native(&["link"]);
    scopes.update(state(WorkspaceScope::Root(reported.clone())));
    let real = NativePath::parse(&native(&["volumes", "data", "project"])).unwrap();
    let inside_real = file(&["volumes", "data", "project", "a"]);
    let reader = connection(1);
    assert_eq!(
        code(view(&scopes, reader).read(&inside_real)),
        Some(ErrorCode::OutsideScope)
    );
    assert_eq!(
        scopes.root(workspace(), |_| Ok(Ok(real.clone()))),
        RootFact {
            generation: 1,
            root: Root::Present(FilePath::new(real.as_str()).unwrap()),
        }
    );
    assert_eq!(code(view(&scopes, reader).read(&inside_real)), None);
    assert_eq!(
        code(view(&scopes, reader).read(&file(&["link", "a"]))),
        None
    );

    let reported = FilePath::new(reported).unwrap();
    assert_eq!(
        scopes
            .root(workspace(), |_| Ok(Err(io::ErrorKind::NotFound.into())))
            .root,
        Root::Missing(reported.clone())
    );
    // A filesystem that does not answer leaves the root as the Workbench reported it.
    assert_eq!(
        scopes
            .root(workspace(), |_| Err(FilesError::new(ErrorCode::Timeout)))
            .root,
        Root::Present(reported)
    );
    // A real location read under a statement the Server replaced meanwhile admits nothing.
    let stale = NativePath::parse(&native(&["stale"])).unwrap();
    let fact = scopes.root(workspace(), |_| {
        scopes.update(state(WorkspaceScope::Root(native(&["moved"]))));
        Ok(Ok(stale.clone()))
    });
    assert_eq!(fact.generation, 1);
    assert_eq!(scopes.generation(workspace()), 2);
    assert_eq!(
        code(view(&scopes, reader).read(&file(&["stale", "a"]))),
        Some(ErrorCode::OutsideScope)
    );
    scopes.update(state(WorkspaceScope::NoRoot));
    assert_eq!(
        scopes.root(workspace(), |_| unreachable!()),
        RootFact {
            generation: 3,
            root: Root::None,
        }
    );
}
