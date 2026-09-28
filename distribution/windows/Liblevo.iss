#define AppName "Liblevo"
#define AppVersion "1.3.0"

[Setup]
AppId={{B11F2D89-386D-42EF-9468-A8F69C329627}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Liblevo contributors
AppPublisherURL=https://github.com/jaimeacena/liblevo
AppSupportURL=https://github.com/jaimeacena/liblevo/issues
AppUpdatesURL=https://github.com/jaimeacena/liblevo/releases/latest
LicenseFile=..\..\LICENSE
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
UsePreviousAppDir=no
UsePreviousGroup=no
OutputDir=..\..\outputs
OutputBaseFilename=Liblevo-Setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
SetupIconFile=..\..\assets\branding\generated\liblevo-app-icon.ico
UninstallDisplayIcon={app}\{#AppName}.exe
UninstallDisplayName={#AppName}
VersionInfoCompany=Liblevo contributors
VersionInfoDescription=Instalador de Liblevo
VersionInfoProductName={#AppName}
VersionInfoVersion={#AppVersion}

[Files]
Source: "..\..\outputs\package\Liblevo\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppName}.exe"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppName}.exe"; Tasks: desktopicon

[InstallDelete]
Type: filesandordirs; Name: "{app}\_internal"

[Tasks]
Name: "desktopicon"; Description: "Crear un acceso directo en el escritorio"; GroupDescription: "Accesos directos:"

[Run]
Filename: "{app}\{#AppName}.exe"; Description: "Abrir {#AppName}"; Flags: nowait postinstall skipifsilent
