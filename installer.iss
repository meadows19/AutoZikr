; AutoZikr Inno Setup Installer Script
#define MyAppName "AutoZikr"
#define MyAppVersion "1.8.2"
#define MyAppPublisher "AutoZikr"
#define MyAppURL "https://github.com/meadows19/AutoZikr"
#define MyAppExeName "autozikr.exe"

[Setup]
AppId={{D37E64B2-9F1B-4A8C-8E2E-15B9A6F7E1D2}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DisableProgramGroupPage=yes
OutputBaseFilename=AutoZikr_Windows_Setup
OutputDir=dist
SetupIconFile=app_icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "startupicon"; Description: "Launch AutoZikr automatically when Windows starts"; GroupDescription: "Startup options:"

[Files]
Source: "target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Dirs]
Name: "{app}\zikr_audio"

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon
Name: "{userstartup}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: startupicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
