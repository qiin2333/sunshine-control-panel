; Sunshine GUI Overlay Installer
; 仅用于把独立 GUI 覆盖安装到已安装的 Sunshine 目录

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-dev"
#endif

#ifndef SourceDir
  #define SourceDir "staging"
#endif

#ifndef OutputDir
  #define OutputDir "dist"
#endif

#define MyAppName "Sunshine GUI Overlay"
#define MyGuiExeName "sunshine-gui.exe"
#define MyIconFile "..\icons\icon.ico"
#define MyChineseLangFile "..\\..\\..\\..\\..\\cmake\\packaging\\ChineseSimplified.isl"

[Setup]
AppId={{0A4CFD8D-9D63-4A31-8E4D-6A17830F2E11}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=AlkaidLab
DefaultDirName={code:GetSunshineGuiDir}
DisableDirPage=yes
DisableProgramGroupPage=yes
CreateUninstallRegKey=no
Uninstallable=no
UsePreviousAppDir=no
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2/ultra64
SolidCompression=yes
OutputDir={#OutputDir}
OutputBaseFilename=Sunshine-GUI-Overlay-{#MyAppVersion}
SetupIconFile={#MyIconFile}
WizardStyle=modern hidebevels
CloseApplications=yes
CloseApplicationsFilter={#MyGuiExeName}
RestartApplications=no
SetupLogging=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimplified"; MessagesFile: "{#MyChineseLangFile}"

[Messages]
english.WelcomeLabel2=This installer updates Sunshine GUI in the existing Sunshine installation.%n%nIt will overwrite {#MyGuiExeName} under assets\gui.
chinesesimplified.WelcomeLabel2=该安装器会把独立 GUI 覆盖到已安装的 Sunshine 中。%n%n它会覆盖 assets\gui 下的 {#MyGuiExeName}。

[Files]
Source: "{#SourceDir}\sunshine-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\WebView2Loader.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Run]
Filename: "{win}\explorer.exe"; Parameters: """{app}\{#MyGuiExeName}"""; Description: "Launch Sunshine GUI"; Flags: postinstall nowait skipifsilent unchecked

[Code]
function QueryInstallDir(RootKey: Integer; const SubKey: String; const ValueName: String; var Value: String): Boolean;
begin
  Result := RegQueryStringValue(RootKey, SubKey, ValueName, Value) and (Value <> '');
end;

function GetSunshineInstallDirInternal(): String;
var
  Dir: String;
begin
  Result := '';

  if QueryInstallDir(HKLM64, 'SOFTWARE\AlkaidLab\Sunshine', 'InstallDir', Dir) then begin
    Result := Dir;
    exit;
  end;

  if QueryInstallDir(HKLM32, 'SOFTWARE\AlkaidLab\Sunshine', 'InstallDir', Dir) then begin
    Result := Dir;
    exit;
  end;

  if QueryInstallDir(HKLM64, 'SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Sunshine_is1', 'InstallLocation', Dir) then begin
    Result := Dir;
    exit;
  end;

  if QueryInstallDir(HKLM32, 'SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Sunshine_is1', 'InstallLocation', Dir) then begin
    Result := Dir;
    exit;
  end;

  Dir := ExpandConstant('{autopf}\Sunshine');
  if FileExists(AddBackslash(Dir) + 'sunshine.exe') then begin
    Result := Dir;
    exit;
  end;

  Dir := ExpandConstant('{pf32}\Sunshine');
  if FileExists(AddBackslash(Dir) + 'sunshine.exe') then begin
    Result := Dir;
    exit;
  end;
end;

function GetSunshineGuiDir(Param: String): String;
var
  InstallDir: String;
begin
  InstallDir := GetSunshineInstallDirInternal();
  if InstallDir = '' then begin
    Result := ExpandConstant('{autopf}\Sunshine\assets\gui');
  end else begin
    Result := AddBackslash(InstallDir) + 'assets\gui';
  end;
end;

function InitializeSetup(): Boolean;
var
  InstallDir: String;
begin
  InstallDir := GetSunshineInstallDirInternal();
  if InstallDir = '' then begin
    MsgBox('未检测到 Sunshine 安装目录。请先安装 Sunshine 主程序，然后再安装独立 GUI。', mbCriticalError, MB_OK);
    Result := False;
    exit;
  end;

  if not FileExists(AddBackslash(InstallDir) + 'sunshine.exe') then begin
    MsgBox('检测到的 Sunshine 路径无效：' + #13#10 + InstallDir, mbCriticalError, MB_OK);
    Result := False;
    exit;
  end;

  Result := True;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
  Result := '';
  // Stop Sunshine service to release file locks
  Exec('net', 'stop SunshineService', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  // Kill running processes
  Exec('taskkill', '/f /im sunshine-gui.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec('taskkill', '/f /im sunshine.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec('taskkill', '/f /im sunshinesvc.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Sleep(1000);
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ResultCode: Integer;
begin
  if CurStep = ssPostInstall then begin
    // Restart Sunshine service after overlay
    Exec('net', 'start SunshineService', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  end;
end;
