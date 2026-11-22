#ifndef MyAppVersion
  #define MyAppVersion "0.3.1"
#endif
#ifndef BinaryPath
  #define BinaryPath "..\..\target\x86_64-pc-windows-msvc\release\krono-desktop.exe"
#endif
#ifndef OutputDirPath
  #define OutputDirPath "..\..\dist"
#endif
#ifndef OutputFileBase
  #define OutputFileBase "krono-v0.3.1-windows-x86_64-setup"
#endif

[Setup]
AppId={{8F3B9C12-4E27-4D8B-9A4C-1B7E2D5F6A30}
AppName=Krono
AppVersion={#MyAppVersion}
AppVerName=Krono {#MyAppVersion}
AppPublisher=Krono
DefaultDirName={autopf}\Krono
DefaultGroupName=Krono
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir={#OutputDirPath}
OutputBaseFilename={#OutputFileBase}
SetupIconFile=..\icons\krono.ico
UninstallDisplayIcon={app}\krono-desktop.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\..\LICENSE

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#BinaryPath}"; DestDir: "{app}"; DestName: "krono-desktop.exe"; Flags: ignoreversion
Source: "..\icons\krono.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.ru.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Krono"; Filename: "{app}\krono-desktop.exe"; IconFilename: "{app}\krono.ico"
Name: "{autodesktop}\Krono"; Filename: "{app}\krono-desktop.exe"; IconFilename: "{app}\krono.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\krono-desktop.exe"; Description: "{cm:LaunchProgram,Krono}"; Flags: nowait postinstall skipifsilent
