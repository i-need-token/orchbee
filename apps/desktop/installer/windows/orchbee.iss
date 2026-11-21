; The per-user Windows installer of the Desktop application. `apps/desktop/scripts/package.mjs`
; compiles it with `--installer` and defines every name below from the distribution profile, so
; this script names no product, folder or identity of its own. `apps/desktop/README.md` describes
; what an install and a removal change on the machine.
;
; An install writes the Start menu shortcut and the uninstall registration, and unpacks the window
; application into Setup's temporary folder. Its bundled Client then runs `orchbee install`, which
; publishes the runtime into the store and owns the user `Path` entry, and the window installer
; helper `orchbee --installer-window-v1`, which installs the window as a version of the store's
; `desktop\` and places the window entry `desktop\<executable>` the shortcut names. A build whose
; window updates are still switched off does not know that helper, so Setup asks it first and, for
; such a build, writes the flat `desktop\` of earlier releases itself, as before. A removal runs
; `orchbee --installer-uninstall-v1`, which removes everything of Orchbee's, the installer's
; shortcut and registration included, and the person's data only when they choose to, and then
; removes the installer's own files. `orchbee uninstall` run in a terminal removes those files too,
; by name: the uninstaller stays `{app}\unins000.exe` with `unins000.dat`, Inno Setup's default.

#if Ver != EncodeVer(6, 7, 1)
  #error This installer is compiled only with Inno Setup 6.7.1, which the release workflow pins.
#endif
#pragma message "Compiler: Inno Setup " + DecodeVer(Ver)

#ifndef AppId
  #error Compile through apps/desktop/scripts/package.mjs --installer, which defines the names.
#endif

[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
VersionInfoVersion={#AppVersionNumeric}
VersionInfoProductTextVersion={#AppVersion}
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\desktop\{#DesktopExecutable}
UninstallDisplaySize={#DisplaySize}
DefaultDirName={code:StoreRoot}
PrivilegesRequired=lowest
; One architecture per installer: the store holds one, and this one carries x86_64 programs.
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0
DisableWelcomePage=yes
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=no
ShowLanguageDialog=no
LanguageDetectionMethod=uilanguage
; Restart Manager would close the programs of a running Session; PrepareToInstall stops instead.
CloseApplications=no
RestartApplications=no
; The bundled `orchbee install` writes the user `Path` entry and leaves the broadcast to Setup.
ChangesEnvironment=yes
SetupIconFile={#IconFile}
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
; The three [Files] entries share one copy of each file.
MergeDuplicateFiles=yes
SetupLogging=yes
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBaseFilename}
OutputManifestFile={#OutputBaseFilename}.files.txt

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "zh"; MessagesFile: "ChineseSimplified.isl"

[CustomMessages]
en.SettingUpCommand=Setting up the %1 command...
zh.SettingUpCommand=正在设置 %1 命令...
en.InstallingWindow=Installing the %1 window...
zh.InstallingWindow=正在安装 %1 窗口程序...
en.SessionInUse=Terminals started by an earlier version of the %1 window are still running from files Setup must replace. Close those terminals, or choose Quit All from the %1 icon in the notification area, then run Setup again. This is needed only once. Nothing was changed.
zh.SessionInUse=较早版本的 %1 窗口启动的终端仍在运行，占用了安装程序需要替换的文件。请关闭这些终端，或在通知区域的 %1 图标上选择“全部退出”，然后重新运行安装程序。此操作只需一次。目前未做任何更改。
en.WindowInUse=%1 is still open; it may be in the notification area. Right-click its icon and quit %1, keeping terminals running. Your terminals and Agents continue. Then run Setup again. Nothing was changed.
zh.WindowInUse=%1 仍在运行，可能在通知区域。请右键单击其图标并退出 %1（终端继续运行），终端和 Agent 不受影响。然后重新运行安装程序。目前未做任何更改。
en.CommandFailed=The %1 window was installed, but the %2 command could not be set up:%n%n%3%n%nRun Setup again to retry.
zh.CommandFailed=%1 窗口程序已安装，但无法设置 %2 命令：%n%n%3%n%n请重新运行安装程序重试。
en.WindowFailed=The %1 window could not be installed:%n%n%2%n%nRun Setup again to retry.
zh.WindowFailed=无法安装 %1 窗口程序：%n%n%2%n%n请重新运行安装程序重试。
en.RemovalBlocked=%1 was not removed:%n%n%2
zh.RemovalBlocked=未能移除 %1：%n%n%2
en.DeleteData=Also delete your %1 data: settings, Sessions and saved passwords?%n%nChoose No to keep it for when you install %1 again.
zh.DeleteData=是否同时删除 %1 的数据，包括设置、会话和保存的密码？%n%n选择“否”会保留这些数据，以后重新安装 %1 时仍可使用。

[Files]
; The bundled Client alone, which Setup asks before installing whether this build installs window
; versions.
Source: "{#SourceDir}\resources\cohort\{#ClientName}.exe"; Flags: dontcopy
; A build that installs window versions: the bundled Client installs the window from here.
Source: "{#SourceDir}\*"; DestDir: "{tmp}\desktop"; Check: WindowStore; Flags: ignoreversion recursesubdirs createallsubdirs uninsneveruninstall
; A build whose window updates are still switched off: the flat layout of earlier releases.
Source: "{#SourceDir}\*"; DestDir: "{app}\desktop"; Check: not WindowStore; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{userprograms}\{#AppName}"; Filename: "{app}\desktop\{#DesktopExecutable}"; AppUserModelID: "{#AppUserModelId}"

[Run]
; Explorer starts the window with the environment the broadcast refreshed, so its terminals find
; the command; Setup's own environment predates the `Path` entry.
; BeforeInstall marks this launch first, so the window it starts switches an open older window to
; the version Setup installed. The entry is skipped when Launch is unticked or the install is silent.
Filename: "{win}\explorer.exe"; Parameters: """{app}\desktop\{#DesktopExecutable}"""; Description: "{cm:LaunchProgram,{#AppName}}"; BeforeInstall: WriteSetupLaunchMarker; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}\desktop.previous"
Type: dirifempty; Name: "{app}"

[Code]
const
  // The bundled Client below a window directory, a signed release directory's own `orchbee`.
  CohortClient = 'resources\cohort\{#ClientName}.exe';
  // The launcher the PATH step installs; it runs the store's active version.
  StoreLauncher = 'bin\{#ClientName}.exe';
  // Where a flat window application waits until the new one is in place.
  PreviousDesktop = 'desktop.previous';
  // Setup's exit code when the window was installed but `orchbee install` failed.
  CommandFailedExitCode = 10;
  // Setup's exit code when the window installer helper failed.
  WindowFailedExitCode = 11;

var
  WindowStoreAsked: Boolean;
  WindowStoreAnswer: Boolean;
  PreviousMoved: Boolean;
  FilesInstalled: Boolean;
  CommandFailed: Boolean;
  CommandDetail: String;
  WindowFailed: Boolean;
  WindowDetail: String;

function SetEnvironmentVariable(Name, Value: String): BOOL;
  external 'SetEnvironmentVariableW@kernel32.dll stdcall';

// The store root by the rule `orchbee install` reads: ORCHBEE_INSTALL_DIR when it is an absolute
// path, otherwise `%LOCALAPPDATA%\Programs\<store folder>`. A previous installation's folder wins.
function StoreRoot(Param: String): String;
var
  Configured: String;
begin
  Configured := GetEnv('ORCHBEE_INSTALL_DIR');
  if ((Length(Configured) >= 3) and (Configured[2] = ':') and (Configured[3] = '\'))
    or ((Length(Configured) >= 3) and (Copy(Configured, 1, 2) = '\\')) then
    Result := RemoveBackslashUnlessRoot(Configured)
  else
    Result := ExpandConstant('{localappdata}\Programs\{#StoreFolderName}');
end;

function JoinLines(const Lines: TArrayOfString): String;
var
  I: Integer;
begin
  Result := '';
  for I := 0 to GetArrayLength(Lines) - 1 do
    if Trim(Lines[I]) <> '' then
    begin
      if Result <> '' then
        Result := Result + #13#10;
      Result := Result + Lines[I];
    end;
end;

// Runs Client from the system folder, so no folder of the store is its working directory. Returns
// the program's own diagnostic, or the system's when it could not start.
function RunProgram(const Client, Params: String; var Code: Integer; var StdOut: String): String;
var
  Output: TExecOutput;
  Started: Boolean;
begin
  Started := False;
  try
    Started := ExecAndCaptureOutput(Client, Params, ExpandConstant('{sys}'), SW_SHOWNORMAL,
      ewWaitUntilTerminated, Code, Output);
  except
    Log(GetExceptionMessage);
  end;
  StdOut := JoinLines(Output.StdOut);
  Result := JoinLines(Output.StdErr);
  if not Started then
  begin
    Result := SysErrorMessage(Code);
    Code := -1;
  end;
  Log(Format('%s %s: exit %d', [Client, Params, Code]));
  Log(StdOut);
  Log(Result);
end;

// Runs an Orchbee Client of this installation with the store root named explicitly.
function RunClient(const Client, Params: String; var Code: Integer; var StdOut: String): String;
begin
  SetEnvironmentVariable('ORCHBEE_INSTALL_DIR', ExpandConstant('{app}'));
  Result := RunProgram(Client, Params, Code, StdOut);
end;

// Whether this build installs window versions, asked once of its bundled Client. A build that does
// not refuses `--installer-window-v1` as an unknown option; one that does answers, from Setup's
// temporary folder, that it does not sit in a window's bundled cohort, and changes nothing.
function WindowStore: Boolean;
var
  Code: Integer;
  StdOut: String;
begin
  if not WindowStoreAsked then
  begin
    WindowStoreAsked := True;
    ExtractTemporaryFile('{#ClientName}.exe');
    RunProgram(ExpandConstant('{tmp}\{#ClientName}.exe'), '--installer-window-v1', Code, StdOut);
    WindowStoreAnswer := (Code = 4) and (Pos('not-release-directory', StdOut) > 0);
    if WindowStoreAnswer then
      Log('This build installs window versions.')
    else
      Log('This build does not install window versions yet; the window goes to desktop\ as it is.');
  end;
  Result := WindowStoreAnswer;
end;

// The error code of a Client's JSON document, or an empty string when it names none.
function DocumentError(const Document: String): String;
var
  Rest: String;
  Start: Integer;
begin
  Result := '';
  Start := Pos('"error"', Document);
  if Start = 0 then
    Exit;
  Rest := Copy(Document, Start + 7, Length(Document));
  Start := Pos('"', Rest);
  if (Start = 0) or (Pos('null', Copy(Rest, 1, Start)) > 0) then
    Exit;
  Rest := Copy(Rest, Start + 1, Length(Rest));
  Start := Pos('"', Rest);
  if Start > 0 then
    Result := Copy(Rest, 1, Start - 1);
end;

// True when the program or library at Path exists and is loaded by a running process. Windows
// refuses to open such a file for writing; opening it changes nothing.
function ProgramInUse(const Path: String): Boolean;
var
  Probe: TFileStream;
begin
  Result := False;
  if not FileExists(Path) then
    Exit;
  try
    Probe := TFileStream.Create(Path, fmOpenReadWrite or fmShareDenyNone);
    Probe.Free;
  except
    Log('In use: ' + Path);
    Result := True;
  end;
end;

// True when a program or library below Dir is loaded by a running process. A folder rename would
// not tell, as Windows lets a folder move while a program runs from it.
function ProgramRunsFrom(const Dir: String): Boolean;
var
  Found: TFindRec;
  Path, Extension: String;
begin
  Result := False;
  if not FindFirst(Dir + '\*', Found) then
    Exit;
  try
    repeat
      Path := Dir + '\' + Found.Name;
      Extension := Lowercase(ExtractFileExt(Found.Name));
      if (Found.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0 then
      begin
        if (Found.Name <> '.') and (Found.Name <> '..')
          and ((Found.Attributes and FILE_ATTRIBUTE_REPARSE_POINT) = 0) then
          Result := ProgramRunsFrom(Path);
      end
      else if (Extension = '.exe') or (Extension = '.dll') or (Extension = '.node') then
        Result := ProgramInUse(Path);
    until Result or not FindNext(Found);
  finally
    FindClose(Found);
  end;
end;

// True when a Session program of the bundled cohort runs. This version's window starts its Session
// from the store, so such a Session began in an earlier version's window or in a start that found
// no usable store version.
function SessionRunsFrom(const Desktop: String): Boolean;
var
  Cohort: String;
begin
  Cohort := Desktop + '\resources\cohort\{#ClientName}';
  Result := ProgramInUse(Cohort + '-server.exe');
  if not Result then
    Result := ProgramInUse(Cohort + '-worker.exe');
  if not Result then
    Result := ProgramInUse(Cohort + '-terminal-host.exe');
end;

// Setup never ends Orchbee. With window versions, it stops before changing anything only while a
// flat layout's window runs: `resources\app.asar`, which only a flat layout has, lies beside the
// entry, and the entry, then that window's executable, is in use. The helper removes no file a
// process runs from, so a Session an earlier window started blocks nothing. Without window
// versions, it stops while that window or such a Session runs from the installed copy and says
// which of the two to quit; otherwise the previous window application moves aside, so no file of
// it lingers beside the new one and a failed install can put it back.
// Changing this (the Setup handover) needs a manual real-window self-test before merging; its
// steps are in apps/desktop/README.md, Window Update, Manual self-test.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Desktop, Previous: String;
begin
  Result := '';
  Desktop := ExpandConstant('{app}\desktop');
  Previous := ExpandConstant('{app}\') + PreviousDesktop;
  if not DirExists(Desktop) then
    Exit;
  if WindowStore then
  begin
    if FileExists(Desktop + '\resources\app.asar')
      and ProgramInUse(Desktop + '\{#DesktopExecutable}') then
      Result := FmtMessage(CustomMessage('WindowInUse'), ['{#AppName}']);
    Exit;
  end;
  if ProgramRunsFrom(Desktop) then
  begin
    if SessionRunsFrom(Desktop) then
      Result := FmtMessage(CustomMessage('SessionInUse'), ['{#AppName}'])
    else
      Result := FmtMessage(CustomMessage('WindowInUse'), ['{#AppName}']);
    Exit;
  end;
  if DirExists(Previous) then
    DelTree(Previous, True, True, True);
  if RenameFile(Desktop, Previous) then
    PreviousMoved := True
  else
    Result := FmtMessage(CustomMessage('WindowInUse'), ['{#AppName}']);
end;

// The bundled Client publishes its signed release directory into the store and activates it; a
// higher version already active stays active, and so does the active one when the bundled version
// is held after a failed move.
procedure InstallCommand(const Client: String);
var
  Code: Integer;
  StdOut: String;
begin
  WizardForm.StatusLabel.Caption := FmtMessage(CustomMessage('SettingUpCommand'), ['{#ClientName}']);
  CommandDetail := RunClient(Client, 'install --format json', Code, StdOut);
  if Code = 0 then
    Exit;
  if (Code = 3) and (Pos('downgrade-refused', StdOut) > 0) then
  begin
    Log('A higher version is active and stays active.');
    Exit;
  end;
  if (Code = 3) and (Pos('version-held', StdOut) > 0) then
  begin
    Log('This version fell back after a failed move and stays held; the active version stays active.');
    Exit;
  end;
  CommandFailed := True;
end;

// The bundled Client installs its window directory as a version of the store's `desktop\`, whatever
// `install` answered, then places the entry, moves a flat layout on and cleans up; a higher window
// version already active stays active. An entry left for a later run because it runs is no
// failure.
procedure InstallWindow(const Client: String);
var
  Code: Integer;
  StdOut, Detail: String;
begin
  WizardForm.StatusLabel.Caption := FmtMessage(CustomMessage('InstallingWindow'), ['{#AppName}']);
  Detail := RunClient(Client, '--installer-window-v1', Code, StdOut);
  if Code = 0 then
    Exit;
  if (Code = 3) and (Pos('downgrade-refused', StdOut) > 0) then
  begin
    Log('A higher window version is active and stays active.');
    Exit;
  end;
  WindowFailed := True;
  WindowDetail := DocumentError(StdOut);
  if WindowDetail = '' then
    WindowDetail := Detail;
end;

// At most one message: a failed window first, because then nothing new starts; otherwise a failed
// command, whose words stay true because the window is in place.
procedure ReportOutcome;
begin
  if WindowFailed then
    SuppressibleMsgBox(FmtMessage(CustomMessage('WindowFailed'), ['{#AppName}', WindowDetail]),
      mbError, MB_OK, IDOK)
  else if CommandFailed then
    SuppressibleMsgBox(FmtMessage(CustomMessage('CommandFailed'), ['{#AppName}', '{#ClientName}',
      CommandDetail]), mbError, MB_OK, IDOK);
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Window: String;
begin
  if CurStep = ssPostInstall then
  begin
    FilesInstalled := True;
    if WindowStore then
    begin
      Window := ExpandConstant('{tmp}\desktop\');
      InstallCommand(Window + CohortClient);
      InstallWindow(Window + CohortClient);
    end
    else
    begin
      if PreviousMoved then
        DelTree(ExpandConstant('{app}\') + PreviousDesktop, True, True, True);
      InstallCommand(ExpandConstant('{app}\desktop\') + CohortClient);
    end;
    ReportOutcome;
  end;
end;

// Setup's own launch of the window switches an open older window to the version it installed, so the
// marker names that version. Without the window store, or without a launch, nothing is written.
procedure WriteSetupLaunchMarker;
begin
  if WindowStore then
    SaveStringToFile(ExpandConstant('{app}\desktop\setup-launch.json'), '{"version":"{#AppVersion}"}', False);
end;

// A cancelled or failed install puts the previous flat window application back.
procedure DeinitializeSetup;
var
  Desktop, Previous: String;
begin
  if not PreviousMoved then
    Exit;
  Desktop := ExpandConstant('{app}\desktop');
  Previous := ExpandConstant('{app}\') + PreviousDesktop;
  if not FilesInstalled and DirExists(Previous) and not DirExists(Desktop) then
    RenameFile(Previous, Desktop);
end;

function GetCustomSetupExitCode: Integer;
begin
  Result := 0;
  if WindowFailed then
    Result := WindowFailedExitCode
  else if CommandFailed then
    Result := CommandFailedExitCode;
end;

// The bundled Client of any window version in the store, or an empty string when none is left.
function WindowVersionClient(const Root: String): String;
var
  Found: TFindRec;
  Versions, Candidate: String;
begin
  Result := '';
  Versions := Root + 'desktop\versions\';
  if not FindFirst(Versions + '*', Found) then
    Exit;
  try
    repeat
      Candidate := Versions + Found.Name + '\' + CohortClient;
      if ((Found.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0)
        and ((Found.Attributes and FILE_ATTRIBUTE_REPARSE_POINT) = 0)
        and (Found.Name <> '.') and (Found.Name <> '..') and FileExists(Candidate) then
        Result := Candidate;
    until (Result <> '') or not FindNext(Found);
  finally
    FindClose(Found);
  end;
end;

// Whether the person deletes their data as well: one question after the uninstaller's own
// confirmation, No by default. A silent removal asks nothing and keeps the data.
function DeleteDataChosen: Boolean;
begin
  Result := False;
  if not UninstallSilent then
    Result := MsgBox(FmtMessage(CustomMessage('DeleteData'), ['{#AppName}']), mbConfirmation,
      MB_YESNO or MB_DEFBUTTON2) = IDYES;
end;

// After the uninstaller's own confirmation, Orchbee's uninstall removes the program files, the
// `Path` entry, this installer's shortcut and registration and the Agent integrations, and keeps
// the person's data unless they chose to delete it. It ends no terminal: with one open it changes
// nothing, data included, and says why, and the removal stops here. The store's active version
// runs it, as it knows every integration a later version added; then a flat window's bundled
// Client, then any window version's; with no Client left, an earlier removal already finished,
// and nothing is asked.
function RemoveInstallation: Boolean;
var
  Root, Client, Params, StdOut, Detail: String;
  Code: Integer;
begin
  Result := True;
  Root := ExpandConstant('{app}\');
  Client := Root + StoreLauncher;
  if not FileExists(Client) then
    Client := Root + 'desktop\' + CohortClient;
  if not FileExists(Client) then
    Client := WindowVersionClient(Root);
  if (Client = '') or not FileExists(Client) then
  begin
    Log('No Orchbee Client is left; nothing of Orchbee''s remains to remove.');
    Exit;
  end;
  Params := '--installer-uninstall-v1';
  if DeleteDataChosen then
    Params := Params + ' --delete-data';
  Detail := RunClient(Client, Params, Code, StdOut);
  if Code = 0 then
    Exit;
  SuppressibleMsgBox(FmtMessage(CustomMessage('RemovalBlocked'), ['{#AppName}', Detail]), mbError,
    MB_OK, IDOK);
  Result := False;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usUninstall) and not RemoveInstallation then
    Abort;
end;
