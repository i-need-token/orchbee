; The per-user Windows installer of the Desktop application. `apps/desktop/scripts/package.mjs`
; compiles it with `--installer` and defines every name below from the distribution profile, so
; this script names no product, folder or identity of its own. `apps/desktop/README.md` describes
; what an install and a removal change on the machine.
;
; An install writes only `desktop\`, the Start menu shortcut and the uninstall registration, then
; runs the bundled `orchbee install`, which publishes the runtime into the store and owns the user
; `Path` entry. A removal runs `orchbee --installer-uninstall-v1`, which removes everything of
; Orchbee's, the installer's shortcut and registration included, and then removes the
; installer's own files. `orchbee uninstall` run in a terminal removes those files too, by name:
; the uninstaller stays `{app}\unins000.exe` with `unins000.dat`, Inno Setup's default.

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
en.DesktopInUse=%1 is still running from this installation, in its window or in terminals it started. Quit %1 and end those terminals, then run Setup again. Nothing was changed.
zh.DesktopInUse=%1 仍在从此安装位置运行（窗口或它启动的终端）。请退出 %1 并结束这些终端，然后重新运行安装程序。目前未做任何更改。
en.CommandFailed=The %1 window was installed, but the %2 command could not be set up:%n%n%3%n%nRun Setup again to retry.
zh.CommandFailed=%1 窗口程序已安装，但无法设置 %2 命令：%n%n%3%n%n请重新运行安装程序重试。
en.RemovalBlocked=%1 was not removed:%n%n%2
zh.RemovalBlocked=未能移除 %1：%n%n%2

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}\desktop"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{userprograms}\{#AppName}"; Filename: "{app}\desktop\{#DesktopExecutable}"; AppUserModelID: "{#AppUserModelId}"

[Run]
; Explorer starts the window with the environment the broadcast refreshed, so its terminals find
; the command; Setup's own environment predates the `Path` entry.
Filename: "{win}\explorer.exe"; Parameters: """{app}\desktop\{#DesktopExecutable}"""; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}\desktop.previous"
Type: dirifempty; Name: "{app}"

[Code]
const
  // The bundled Client, a signed release directory's own `orchbee`.
  BundledClient = 'desktop\resources\cohort\{#ClientName}.exe';
  // The launcher the PATH step installs; it runs the store's active version.
  StoreLauncher = 'bin\{#ClientName}.exe';
  // Where the previous window application waits until the new one is in place.
  PreviousDesktop = 'desktop.previous';
  // Setup's exit code when the window was installed but `orchbee install` failed.
  CommandFailedExitCode = 10;

var
  PreviousMoved: Boolean;
  FilesInstalled: Boolean;
  CommandFailed: Boolean;

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

// Runs an Orchbee Client of this installation from the system folder, so no folder of the store
// is its working directory, with the store root named explicitly. Returns the program's own
// diagnostic, or the system's when it could not start.
function RunClient(const Client, Params: String; var Code: Integer; var StdOut: String): String;
var
  Output: TExecOutput;
  Started: Boolean;
begin
  SetEnvironmentVariable('ORCHBEE_INSTALL_DIR', ExpandConstant('{app}'));
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

// True when a program or library below Dir is loaded by a running process. Windows refuses to
// open such a file for writing; opening it changes nothing. A folder rename would not tell, as
// Windows lets a folder move while a program runs from it.
function ProgramRunsFrom(const Dir: String): Boolean;
var
  Found: TFindRec;
  Path, Extension: String;
  Probe: TFileStream;
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
      begin
        try
          Probe := TFileStream.Create(Path, fmOpenReadWrite or fmShareDenyNone);
          Probe.Free;
        except
          Log('In use: ' + Path);
          Result := True;
        end;
      end;
    until Result or not FindNext(Found);
  finally
    FindClose(Found);
  end;
end;

// Setup never ends Orchbee: while its window or a Session it started runs from the installed
// copy, Setup stops before changing anything. Otherwise the previous window application moves
// aside, so no file of it lingers beside the new one and a failed install can put it back.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Desktop, Previous: String;
begin
  Result := '';
  Desktop := ExpandConstant('{app}\desktop');
  Previous := ExpandConstant('{app}\') + PreviousDesktop;
  if not DirExists(Desktop) then
    Exit;
  if ProgramRunsFrom(Desktop) then
  begin
    Result := FmtMessage(CustomMessage('DesktopInUse'), ['{#AppName}']);
    Exit;
  end;
  if DirExists(Previous) then
    DelTree(Previous, True, True, True);
  if RenameFile(Desktop, Previous) then
    PreviousMoved := True
  else
    Result := FmtMessage(CustomMessage('DesktopInUse'), ['{#AppName}']);
end;

// The bundled Client publishes its signed release directory into the store and activates it; a
// higher version already active stays active.
procedure InstallCommand;
var
  Code: Integer;
  StdOut, Detail: String;
begin
  WizardForm.StatusLabel.Caption := FmtMessage(CustomMessage('SettingUpCommand'), ['{#ClientName}']);
  Detail := RunClient(ExpandConstant('{app}\') + BundledClient, 'install --format json', Code, StdOut);
  if Code = 0 then
    Exit;
  if (Code = 3) and (Pos('downgrade-refused', StdOut) > 0) then
  begin
    Log('A higher version is active and stays active.');
    Exit;
  end;
  CommandFailed := True;
  SuppressibleMsgBox(FmtMessage(CustomMessage('CommandFailed'), ['{#AppName}', '{#ClientName}', Detail]),
    mbError, MB_OK, IDOK);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    FilesInstalled := True;
    if PreviousMoved then
      DelTree(ExpandConstant('{app}\') + PreviousDesktop, True, True, True);
    InstallCommand;
  end;
end;

// A cancelled or failed install puts the previous window application back.
procedure DeinitializeSetup;
var
  Desktop, Previous: String;
begin
  Desktop := ExpandConstant('{app}\desktop');
  Previous := ExpandConstant('{app}\') + PreviousDesktop;
  if PreviousMoved and not FilesInstalled and DirExists(Previous) and not DirExists(Desktop) then
    RenameFile(Previous, Desktop);
end;

function GetCustomSetupExitCode: Integer;
begin
  Result := 0;
  if CommandFailed then
    Result := CommandFailedExitCode;
end;

// After the uninstaller's own confirmation, Orchbee's uninstall removes the program files, the
// `Path` entry, this installer's shortcut and registration and the Agent integrations, and keeps
// the person's data. It ends no terminal: with one open it changes nothing and says why, and
// the removal stops here. The store's active version runs it, as it knows every integration a
// later version added; with no Client left, an earlier removal already finished.
function RemoveInstallation: Boolean;
var
  Root, Client, StdOut, Detail: String;
  Code: Integer;
begin
  Result := True;
  Root := ExpandConstant('{app}\');
  Client := Root + StoreLauncher;
  if not FileExists(Client) then
    Client := Root + BundledClient;
  if not FileExists(Client) then
  begin
    Log('No Orchbee Client is left; nothing of Orchbee''s remains to remove.');
    Exit;
  end;
  Detail := RunClient(Client, '--installer-uninstall-v1', Code, StdOut);
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
