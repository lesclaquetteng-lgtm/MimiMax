; CUBE Launcher — custom NSIS template.
;
; Replaces the stock Tauri assistant with a borderless CUBE window: brand rail on the left, four
; dark steps (accueil, licence, options, progression, fin), custom buttons and progress bar. The
; installation work itself (files, registry, shortcuts, WebView2) still follows the Tauri bundle,
; so silent installs (`/S`) and update runs (`/UPDATE`) keep working unchanged.

Unicode true
ManifestDPIAware true
ManifestDPIAwareness PerMonitorV2

!if "{{compression}}" == "none"
SetCompress off
!else
SetCompressor /SOLID "{{compression}}"
!endif

{{#if signed_plugins_path}}
!addplugindir "{{signed_plugins_path}}"
{{/if}}

!include MUI2.nsh
!include FileFunc.nsh
!include x64.nsh
!include WordFunc.nsh
!include LogicLib.nsh
!include nsDialogs.nsh
!include WinMessages.nsh
!include "utils.nsh"
!include "FileAssociation.nsh"
!include "Win\COM.nsh"
!include "Win\Propkey.nsh"
!include "Win\RestartManager.nsh"
!include "StrFunc.nsh"
${StrCase}
${StrLoc}

{{#if installer_hooks}}
!include "{{installer_hooks}}"
{{/if}}

!define WEBVIEW2APPGUID "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
!define MANUFACTURER "{{manufacturer}}"
!define PRODUCTNAME "{{product_name}}"
!define VERSION "{{version}}"
!define VERSIONWITHBUILD "{{version_with_build}}"
!define HOMEPAGE "{{homepage}}"
!define INSTALLMODE "{{install_mode}}"
!define LICENSE "{{license}}"

!define INSTALLERICON "{{installer_icon}}"
!define SIDEBARIMAGE "{{sidebar_image}}"
!define HEADERIMAGE "{{header_image}}"
!define UNINSTALLERICON "{{uninstaller_icon}}"
!define UNINSTALLERHEADERIMAGE "{{uninstaller_header_image}}"
!define MAINBINARYNAME "{{main_binary_name}}"
!define MAINBINARYSRCPATH "{{main_binary_path}}"
!define BUNDLEID "{{bundle_id}}"
!define COPYRIGHT "{{copyright}}"
!define OUTFILE "{{out_file}}"
!define ARCH "{{arch}}"
!define ADDITIONALPLUGINSPATH "{{additional_plugins_path}}"
!define ALLOWDOWNGRADES "{{allow_downgrades}}"
!define DISPLAYLANGUAGESELECTOR "{{display_language_selector}}"
!define INSTALLWEBVIEW2MODE "{{install_webview2_mode}}"
!define WEBVIEW2INSTALLERARGS "{{webview2_installer_args}}"
!define WEBVIEW2BOOTSTRAPPERPATH "{{webview2_bootstrapper_path}}"
!define WEBVIEW2INSTALLERPATH "{{webview2_installer_path}}"
!define MINIMUMWEBVIEW2VERSION "{{minimum_webview2_version}}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}"
!define MANUKEY "Software\${MANUFACTURER}"
!define MANUPRODUCTKEY "${MANUKEY}\${PRODUCTNAME}"
!define UNINSTALLERSIGNCOMMAND "{{uninstaller_sign_cmd}}"
!define ESTIMATEDSIZE "{{estimated_size}}"
!define STARTMENUFOLDER "{{start_menu_folder}}"

; ------------------------------------------------------------------ the palette
; SetCtlColors takes 0xRRGGBB values here (verified on the rendered pages).
!define CLR_TEXT 0xF2F4F6
!define CLR_MUTED 0x8A938C
!define CLR_DIM 0x5E6660
!define CLR_BRAND 0x93E05C
!define CLR_INK 0x08110A
!define CLR_SURFACE 0x171A1F
!define CLR_BG 0x0B0D10
!define CLR_BG_INPUT 0x121418
!define CLR_FIELD 0x272C33
!define ACCENT_MS 18

!ifndef PBM_SETPOS
!define PBM_SETPOS 0x0402
!endif
!ifndef PBM_SETBARCOLOR
!define PBM_SETBARCOLOR 0x0409
!endif
!ifndef PBM_SETBKCOLOR
!define PBM_SETBKCOLOR 0x2001
!endif
!ifndef STM_SETIMAGE
!define STM_SETIMAGE 0x0172
!endif
!ifndef STM_SETICON
!define STM_SETICON 0x0170
!endif
!ifndef IMAGE_BITMAP
!define IMAGE_BITMAP 0
!endif
!ifndef IMAGE_ICON
!define IMAGE_ICON 1
!endif

Var PassiveMode
Var UpdateMode
Var NoShortcutMode
Var WixMode
Var OldMainBinaryName
Var AppStartMenuFolder
Var CreateDesktop
Var RunAfterInstall

; ------------------------------------------------------------------- the window
Var hwndPage
Var hBack
Var hProg
Var hFont
Var ProgressValue
Var ProgressStatus

; one handle per control, grouped by step
Var hKicker0
Var hTitle0
Var hSub0
Var hSub0b
Var hFeat1
Var hFeat1t
Var hFeat2
Var hFeat2t
Var hFeat3
Var hFeat3t
Var hVer
Var hCancel
Var hInstall0

Var hKicker1
Var hTitle1
Var hLicense
Var hAccept
Var AcceptOn
Var hBack1
Var hNext1

Var hKicker2
Var hTitle2
Var hPathLabel
Var hPath
Var hBrowse
Var hFieldT
Var hFieldB
Var hFieldL
Var hFieldR
Var DesktopOn
Var LaunchOn
Var hDesktop
Var hLaunch
Var hInfo
Var hBack2
Var hInstall2

Var hStatus
Var hPercent

Var hKicker4
Var hTitle4
Var hDone1
Var hDone2
Var hFinPath
Var hFinish

Var hAccent
Var hClose
Var hDrag

Var LicenseText

Name "${PRODUCTNAME}"
BrandingText " "
OutFile "${OUTFILE}"

!define PLACEHOLDER_INSTALL_DIR "placeholder\${PRODUCTNAME}"
InstallDir "${PLACEHOLDER_INSTALL_DIR}"

VIProductVersion "${VERSIONWITHBUILD}"
VIAddVersionKey "ProductName" "${PRODUCTNAME}"
VIAddVersionKey "FileDescription" "${PRODUCTNAME}"
VIAddVersionKey "LegalCopyright" "${COPYRIGHT}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"

!addplugindir "${ADDITIONALPLUGINSPATH}"

!if "${UNINSTALLERSIGNCOMMAND}" != ""
!uninstfinalize '${UNINSTALLERSIGNCOMMAND}'
!endif

!if "${INSTALLMODE}" == "perMachine"
RequestExecutionLevel admin
!endif
!if "${INSTALLMODE}" == "currentUser"
RequestExecutionLevel user
!endif

; The installer UI is fully custom, so MUI only brands the uninstaller.
!if "${INSTALLERICON}" != ""
!define MUI_ICON "${INSTALLERICON}"
!endif
!if "${SIDEBARIMAGE}" != ""
!define MUI_WELCOMEFINISHPAGE_BITMAP "${SIDEBARIMAGE}"
!endif
!if "${HEADERIMAGE}" != ""
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_BITMAP "${HEADERIMAGE}"
!else if "${UNINSTALLERHEADERIMAGE}" != ""
!define MUI_HEADERIMAGE
!endif
!if "${UNINSTALLERHEADERIMAGE}" != ""
!define MUI_HEADERIMAGE_UNBITMAP "${UNINSTALLERHEADERIMAGE}"
!endif
!if "${UNINSTALLERICON}" != ""
!define MUI_UNICON "${UNINSTALLERICON}"
!endif

!define MUI_LANGDLL_REGISTRY_ROOT "HKCU"
!define MUI_LANGDLL_REGISTRY_KEY "${MANUPRODUCTKEY}"
!define MUI_LANGDLL_REGISTRY_VALUENAME "Installer Language"

!define MUI_CUSTOMFUNCTION_GUIINIT MyGuiInit

; -------------------------------------------------------------------- the pages
Page custom PageBrand
!define MUI_PAGE_CUSTOMFUNCTION_SHOW un.ConfirmShow
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE un.ConfirmLeave
!define MUI_PAGE_CUSTOMFUNCTION_PRE un.SkipIfPassive
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

{{#each languages}}
!insertmacro MUI_LANGUAGE "{{this}}"
{{/each}}
!insertmacro MUI_RESERVEFILE_LANGDLL
{{#each language_files}}
!include "{{this}}"
{{/each}}

; --------------------------------------------------------------------- install
Function .onInit
	${GetOptions} $CMDLINE "/P" $PassiveMode
	${IfNot} ${Errors}
		StrCpy $PassiveMode 1
	${EndIf}
	${GetOptions} $CMDLINE "/NS" $NoShortcutMode
	${IfNot} ${Errors}
		StrCpy $NoShortcutMode 1
	${EndIf}
	${GetOptions} $CMDLINE "/UPDATE" $UpdateMode
	${IfNot} ${Errors}
		StrCpy $UpdateMode 1
	${EndIf}

	!if "${DISPLAYLANGUAGESELECTOR}" == "true"
	!insertmacro MUI_LANGDLL_DISPLAY
	!endif

	!insertmacro SetContext

	${If} $INSTDIR == "${PLACEHOLDER_INSTALL_DIR}"
		!if "${INSTALLMODE}" == "perMachine"
		${If} ${RunningX64}
			!if "${ARCH}" == "x64"
			StrCpy $INSTDIR "$PROGRAMFILES64\${PRODUCTNAME}"
			!else if "${ARCH}" == "arm64"
			StrCpy $INSTDIR "$PROGRAMFILES64\${PRODUCTNAME}"
			!else
			StrCpy $INSTDIR "$PROGRAMFILES32\${PRODUCTNAME}"
			!endif
		${Else}
			StrCpy $INSTDIR "$PROGRAMFILES\${PRODUCTNAME}"
		${EndIf}
		!else if "${INSTALLMODE}" == "currentUser"
		StrCpy $INSTDIR "$LOCALAPPDATA\${PRODUCTNAME}"
		!endif
		Call RestorePreviousInstallLocation
	${EndIf}

	StrCpy $AppStartMenuFolder "${STARTMENUFOLDER}"
	StrCpy $CreateDesktop 1
	StrCpy $RunAfterInstall 1

	${If} ${Silent}
		StrCpy $RunAfterInstall 0
	${EndIf}
	${If} $PassiveMode = 1
		StrCpy $RunAfterInstall 0
	${EndIf}
FunctionEnd

Section EarlyChecks
!if "${ALLOWDOWNGRADES}" == "false"
	${If} ${Silent}
		${If} $R0 = -1
			System::Call 'kernel32::AttachConsole(i -1)i.r0'
			${If} $0 <> 0
				System::Call 'kernel32::GetStdHandle(i -11)i.r0'
				System::call 'kernel32::SetConsoleTextAttribute(i r0, i 0x0004)'
				FileWrite $0 "$(silentDowngrades)"
			${EndIf}
			Abort
		${EndIf}
	${EndIf}
!endif
SectionEnd

Section WebView2
	Call InstallWebView2
SectionEnd

Section Install
	Call InstallCore
SectionEnd

!define PROGRESS_HOLD_MS 130

Var Step
Var Installing

; ------------------------------------------------------------------- the widgets
!macro Reveal VAR
	ShowWindow ${VAR} 1
	System::Call 'user32::InvalidateRect(p ${VAR}, p 0, i 1)'
	System::Call 'user32::UpdateWindow(p ${VAR})'
!macroend

!macro Lbl VAR X Y W H TEXT SIZE WEIGHT COLOR
	${NSD_CreateLabel} ${X} ${Y} ${W} ${H} "${TEXT}"
	Pop ${VAR}
	CreateFont $hFont "Segoe UI" "${SIZE}" "${WEIGHT}"
	SendMessage ${VAR} ${WM_SETFONT} $hFont 1
	SetCtlColors ${VAR} ${COLOR} ${CLR_BG}
!macroend

!macro Kick VAR X Y W TEXT
	${NSD_CreateLabel} ${X} ${Y} ${W} 12 "${TEXT}"
	Pop ${VAR}
	CreateFont $hFont "Consolas" "9" "700"
	SendMessage ${VAR} ${WM_SETFONT} $hFont 1
	SetCtlColors ${VAR} ${CLR_BRAND} ${CLR_BG}
!macroend

; A hairline: a plain static painted with its background colour, so it follows the palette.
!macro Rule VAR X Y W H COLOR
	nsDialogs::CreateControl STATIC "0x54000000" 0 ${X} ${Y} ${W} ${H} ""
	Pop ${VAR}
	SetCtlColors ${VAR} ${COLOR} ${COLOR}
!macroend

!macro Btn VAR X Y W H TEXT CLICK PRIMARY
	nsDialogs::CreateControl STATIC "0x54000301" 0 ${X} ${Y} ${W} ${H} "${TEXT}"
	Pop ${VAR}
	CreateFont $hFont "Segoe UI" "10" "700"
	SendMessage ${VAR} ${WM_SETFONT} $hFont 1
	!if ${PRIMARY} == 1
	SetCtlColors ${VAR} ${CLR_INK} ${CLR_BRAND}
	!else
	SetCtlColors ${VAR} ${CLR_TEXT} ${CLR_SURFACE}
	!endif
	${NSD_OnClick} ${VAR} ${CLICK}
!macroend

Function MyGuiInit
	; No Windows caption: the brand rail and the close button are the only frame.
	System::Call 'user32::GetWindowLong(p $HWNDPARENT, i -16) i .r0'
	IntOp $0 $0 & 0xFF33FFFF
	System::Call 'user32::SetWindowLong(p $HWNDPARENT, i -16, i r0)'

FunctionEnd

Function PageBrand
	nsDialogs::Create 760 440
	Pop $hwndPage
	${If} $hwndPage == error
		Abort
	${EndIf}

	System::Call 'user32::SetWindowPos(p $HWNDPARENT, p 0, i 0, i 0, i 760, i 440, i 0x16)'
	System::Call 'user32::MoveWindow(p $hwndPage, i 0, i 0, i 760, i 440, i 0)'
	; without WS_CLIPCHILDREN the page background erases the controls on every repaint
	System::Call 'user32::GetWindowLong(p $hwndPage, i -16) i .r0'
	IntOp $0 $0 | 0x02000000
	System::Call 'user32::SetWindowLong(p $hwndPage, i -16, i r0)'
	GetDlgItem $0 $HWNDPARENT 1
	ShowWindow $0 0
	GetDlgItem $0 $HWNDPARENT 2
	ShowWindow $0 0
	GetDlgItem $0 $HWNDPARENT 3
	ShowWindow $0 0

	; ------------------------------------------------------- background art
	InitPluginsDir
	File /oname=$PLUGINSDIR\panel.bmp "${SIDEBARIMAGE}"
	System::Call 'user32::LoadImage(p 0, t "$PLUGINSDIR\panel.bmp", i 0, i 0, i 0, i 0x0010) p .r1'
	nsDialogs::CreateControl STATIC "0x5400000E" 0 0 0 760 440 ""
	Pop $hBack
	${If} $r1 != 0
		System::Call 'user32::SendMessage(p $hBack, i 0x0172, p 0, p r1)'
	${EndIf}

	; ------------------------------------------------------------- title bar
	!insertmacro Lbl $hDrag 232 0 472 48 "" 9 400 ${CLR_BG}
	${NSD_OnClick} $hDrag OnDragWindow

	nsDialogs::CreateControl STATIC "0x50000301" 0 712 8 36 32 "✕"
	Pop $hClose
	CreateFont $hFont "Segoe UI" "12" "400"
	SendMessage $hClose ${WM_SETFONT} $hFont 1
	SetCtlColors $hClose ${CLR_MUTED} ${CLR_BG}
	${NSD_OnClick} $hClose OnClose

	; the transition tick shared by every state, under the kicker line
	!insertmacro Rule $hAccent 264 68 44 2 ${CLR_BRAND}
	ShowWindow $hAccent 0

	; ------------------------------------------------------------ step 0: accueil
	!insertmacro Kick $hKicker0 264 48 440 "CUBE LAUNCHER  /  VOXEL EDITION"
	!insertmacro Lbl $hTitle0 264 76 440 42 "Ton aventure commence ici" 23 700 ${CLR_TEXT}
	!insertmacro Lbl $hSub0 264 128 440 18 "Minecraft, Fabric et tes mondes réunis." 11 400 ${CLR_MUTED}
	!insertmacro Lbl $hSub0b 264 150 440 18 "Une installation simple. Puis, à toi de jouer." 10 400 ${CLR_MUTED}

	!insertmacro Lbl $hFeat1 264 202 14 14 "✦" 9 400 ${CLR_BRAND}
	!insertmacro Lbl $hFeat1t 284 198 400 20 "Tes comptes et tes profils Minecraft" 10 400 ${CLR_TEXT}
	!insertmacro Lbl $hFeat2 264 232 14 14 "✦" 9 400 ${CLR_BRAND}
	!insertmacro Lbl $hFeat2t 284 228 400 20 "Fabric, mods et serveurs entre amis" 10 400 ${CLR_TEXT}
	!insertmacro Lbl $hFeat3 264 262 14 14 "✦" 9 400 ${CLR_BRAND}
	!insertmacro Lbl $hFeat3t 284 258 400 20 "Un launcher léger, prêt à jouer" 10 400 ${CLR_TEXT}
	!insertmacro Lbl $hVer 264 310 440 18 "VERSION ${VERSION}     ·     WINDOWS 10 / 11" 8 400 ${CLR_DIM}

	!insertmacro Btn $hCancel 424 372 112 40 "ANNULER" OnClose 0
	!insertmacro Btn $hInstall0 548 372 176 40 "INSTALLER" OnGoLicense 1

	; ------------------------------------------------------------- step 1: licence
	!insertmacro Kick $hKicker1 264 54 440 "ÉTAPE 1 SUR 3 / LICENCE"
	!insertmacro Lbl $hTitle1 264 76 440 40 "Avant de commencer" 23 700 ${CLR_TEXT}

	StrCpy $LicenseText ""
	File /oname=$PLUGINSDIR\license.txt "${LICENSE}"
	FileOpen $0 "$PLUGINSDIR\license.txt" r
	${If} $0 != ""
		lic_loop:
			ClearErrors
			FileRead $0 $1
			IfErrors lic_done
			StrCpy $LicenseText "$LicenseText$1"
			Goto lic_loop
		lic_done:
			FileClose $0
	${EndIf}

	; a label sheet instead of an EDIT: the page dialog proc never paints an EDIT here
	; LICENSE-FR.txt is stored in Windows-1252 so FileRead returns readable accents.
	; tauri writes the licence copy with a UTF-8 BOM, read here as three ANSI chars
	StrCpy $0 $LicenseText 3
	${If} $0 == "ï»¿"
		StrCpy $LicenseText $LicenseText "" 3
	${EndIf}
	${NSD_CreateLabel} 264 120 440 228 "$LicenseText"
	Pop $hLicense
	CreateFont $hFont "Consolas" "8" "400"
	SendMessage $hLicense ${WM_SETFONT} $hFont 1
	SetCtlColors $hLicense ${CLR_MUTED} ${CLR_BG}

	!insertmacro Lbl $hAccept 264 352 440 20 "☐   J'accepte le contrat de licence" 10 400 ${CLR_MUTED}
	StrCpy $AcceptOn 0
	${NSD_OnClick} $hAccept OnAcceptToggle

	!insertmacro Btn $hBack1 424 372 112 40 "RETOUR" OnBackWelcome 0
	!insertmacro Btn $hNext1 548 372 176 40 "CONTINUER" OnGoOptions 1
	EnableWindow $hNext1 0
	SetCtlColors $hNext1 ${CLR_MUTED} ${CLR_SURFACE}

	; ------------------------------------------------------------- step 2: options
	!insertmacro Kick $hKicker2 264 54 440 "ÉTAPE 2 SUR 3 / DOSSIER"
	!insertmacro Lbl $hTitle2 264 76 440 40 "Choisis ton emplacement" 23 700 ${CLR_TEXT}
	!insertmacro Lbl $hPathLabel 264 134 440 16 "Dossier d'installation" 9 600 ${CLR_MUTED}

	; Drawn folder field: a 1 px frame around a dark borderless EDIT, so the page keeps
	; the palette instead of falling back to a white Windows text box.
	!insertmacro Rule $hFieldT 264 154 348 1 ${CLR_FIELD}
	!insertmacro Rule $hFieldB 264 181 348 1 ${CLR_FIELD}
	!insertmacro Rule $hFieldL 264 154 1 28 ${CLR_FIELD}
	!insertmacro Rule $hFieldR 611 154 1 28 ${CLR_FIELD}

	nsDialogs::CreateControl "EDIT" "0x50010080" 0 276 159 322 18 "$INSTDIR"
	Pop $hPath
	System::Call 'uxtheme::SetWindowTheme(p $hPath, w "", w "")'
	CreateFont $hFont "Segoe UI" "10" "400"
	SendMessage $hPath ${WM_SETFONT} $hFont 1
	SetCtlColors $hPath ${CLR_TEXT} ${CLR_BG_INPUT}

	!insertmacro Btn $hBrowse 612 150 92 32 "PARCOURIR" OnBrowse 0

	!insertmacro Lbl $hDesktop 264 204 440 20 "☑   Créer un raccourci sur le bureau" 10 400 ${CLR_TEXT}
	StrCpy $DesktopOn 1
	${NSD_OnClick} $hDesktop OnDesktopToggle

	!insertmacro Lbl $hLaunch 264 232 440 20 "☑   Lancer CUBE Launcher à la fin de l'installation" 10 400 ${CLR_TEXT}
	StrCpy $LaunchOn 1
	${NSD_OnClick} $hLaunch OnLaunchToggle

	!insertmacro Lbl $hInfo 264 276 440 30 "Installation personnelle · aucun droit administrateur requis." 9 400 ${CLR_DIM}

	!insertmacro Btn $hBack2 424 372 112 40 "RETOUR" OnGoLicense 0
	!insertmacro Btn $hInstall2 548 372 176 40 "INSTALLER" OnInstallClick 1

	; ---------------------------------------------------------- step 3: progression
	!insertmacro Lbl $hPercent 264 96 440 56 "0 %" 40 700 ${CLR_TEXT}
	!insertmacro Lbl $hStatus 264 160 440 20 "Préparation…" 11 400 ${CLR_MUTED}

	nsDialogs::CreateControl "msctls_progress32" "0x50000001" 0 264 196 440 6 ""
	Pop $hProg
	SendMessage $hProg ${PBM_SETBKCOLOR} 0 ${CLR_SURFACE}
	SendMessage $hProg ${PBM_SETBARCOLOR} 0 ${CLR_BRAND}

	; ---------------------------------------------------------------- step 4: fin
	!insertmacro Kick $hKicker4 264 54 440 "TERMINÉ"
	!insertmacro Lbl $hTitle4 264 76 440 40 "Tout est prêt !" 23 700 ${CLR_TEXT}
	!insertmacro Lbl $hDone1 264 132 440 18 "CUBE Launcher est installé sur ton ordinateur." 11 400 ${CLR_TEXT}
	!insertmacro Lbl $hDone2 264 156 440 18 "Les mises à jour se feront depuis le launcher." 10 400 ${CLR_MUTED}
	!insertmacro Lbl $hFinPath 264 196 440 18 "" 9 400 ${CLR_DIM}

	!insertmacro Btn $hFinish 548 372 176 40 "TERMINER" OnFinish 1

	; The art must sit under every control: push it to the bottom of the z-order last.
	System::Call 'user32::SetWindowPos(p $hBack, p 1, i 0, i 0, i 0, i 0, i 0x13)'

	; /P and /UPDATE open straight on the progress sheet, then start themselves once the page
	; is up: the page only becomes visible after this function returns, hence the timer.
	${If} $PassiveMode = 1
	${OrIf} $UpdateMode = 1
		StrCpy $Step 3
		StrCpy $ProgressValue 3
		StrCpy $ProgressStatus "Préparation du dossier…"
		Call SetProgress
	${Else}
		StrCpy $Step 0
	${EndIf}
	Call ShowStep

	${If} $PassiveMode = 1
	${OrIf} $UpdateMode = 1
		${NSD_CreateTimer} RunUnattended 120
	${EndIf}
	nsDialogs::Show
FunctionEnd

; ------------------------------------------------------------------ step engine
!ifndef WM_NCLBUTTONDOWN
!define WM_NCLBUTTONDOWN 0x00A1
!endif
!ifndef HTCAPTION
!define HTCAPTION 2
!endif

Function HideStepControls
	ShowWindow $hKicker0 0
	ShowWindow $hTitle0 0
	ShowWindow $hSub0 0
	ShowWindow $hSub0b 0
	ShowWindow $hFeat1 0
	ShowWindow $hFeat1t 0
	ShowWindow $hFeat2 0
	ShowWindow $hFeat2t 0
	ShowWindow $hFeat3 0
	ShowWindow $hFeat3t 0
	ShowWindow $hVer 0
	ShowWindow $hCancel 0
	ShowWindow $hInstall0 0
	ShowWindow $hKicker1 0
	ShowWindow $hTitle1 0
	ShowWindow $hLicense 0
	ShowWindow $hAccept 0
	ShowWindow $hBack1 0
	ShowWindow $hNext1 0
	ShowWindow $hKicker2 0
	ShowWindow $hTitle2 0
	ShowWindow $hPathLabel 0
	ShowWindow $hPath 0
	ShowWindow $hBrowse 0
	ShowWindow $hDesktop 0
	ShowWindow $hLaunch 0
	ShowWindow $hInfo 0
	ShowWindow $hBack2 0
	ShowWindow $hInstall2 0
	ShowWindow $hFieldT 0
	ShowWindow $hFieldB 0
	ShowWindow $hFieldL 0
	ShowWindow $hFieldR 0
	ShowWindow $hAccent 0
	ShowWindow $hStatus 0
	ShowWindow $hPercent 0
	ShowWindow $hProg 0
	ShowWindow $hKicker4 0
	ShowWindow $hTitle4 0
	ShowWindow $hDone1 0
	ShowWindow $hDone2 0
	ShowWindow $hFinPath 0
	ShowWindow $hFinish 0
	; hidden controls leave their pixels behind: repaint the art before revealing the next step
	System::Call 'user32::InvalidateRect(p $hBack, p 0, i 1)'
	System::Call 'user32::UpdateWindow(p $hBack)'
FunctionEnd

Function ShowStep
	Call HideStepControls
	${If} $Step = 0
		!insertmacro Reveal $hKicker0
		!insertmacro Reveal $hTitle0
		!insertmacro Reveal $hSub0
		!insertmacro Reveal $hSub0b
		!insertmacro Reveal $hFeat1
		!insertmacro Reveal $hFeat1t
		!insertmacro Reveal $hFeat2
		!insertmacro Reveal $hFeat2t
		!insertmacro Reveal $hFeat3
		!insertmacro Reveal $hFeat3t
		!insertmacro Reveal $hVer
		!insertmacro Reveal $hCancel
		!insertmacro Reveal $hInstall0
	${ElseIf} $Step = 1
		!insertmacro Reveal $hKicker1
		!insertmacro Reveal $hTitle1
		!insertmacro Reveal $hLicense
		!insertmacro Reveal $hAccept
		!insertmacro Reveal $hBack1
		!insertmacro Reveal $hNext1
	${ElseIf} $Step = 2
		!insertmacro Reveal $hKicker2
		!insertmacro Reveal $hTitle2
		!insertmacro Reveal $hFieldT
		!insertmacro Reveal $hFieldB
		!insertmacro Reveal $hFieldL
		!insertmacro Reveal $hFieldR
		!insertmacro Reveal $hPathLabel
		!insertmacro Reveal $hPath
		!insertmacro Reveal $hBrowse
		!insertmacro Reveal $hDesktop
		!insertmacro Reveal $hLaunch
		!insertmacro Reveal $hInfo
		!insertmacro Reveal $hBack2
		!insertmacro Reveal $hInstall2
	${ElseIf} $Step = 3
		!insertmacro Reveal $hPercent
		!insertmacro Reveal $hStatus
		!insertmacro Reveal $hProg
	${Else}
		!insertmacro Reveal $hKicker4
		!insertmacro Reveal $hTitle4
		!insertmacro Reveal $hDone1
		!insertmacro Reveal $hDone2
		!insertmacro Reveal $hFinPath
		!insertmacro Reveal $hFinish
	${EndIf}
	Call PulseAccent
FunctionEnd

; The only motion between states: a 2 px brand tick that grows under the kicker. Three
; frames, one small control, so the cost stays negligible next to a forced page repaint.
Function PulseAccent
	Push $0
	${If} $hAccent != 0
		StrCpy $0 0
		ShowWindow $hAccent 1
		${Do}
			System::Call 'user32::SetWindowPos(p $hAccent, p 0, i 264, i 68, i $0, i 2, i 0x14)'
			System::Call 'user32::InvalidateRect(p $hAccent, p 0, i 1)'
			System::Call 'user32::UpdateWindow(p $hAccent)'
			Sleep ${ACCENT_MS}
			IntOp $0 $0 + 8
		${LoopUntil} $0 > 40
	${EndIf}
	Pop $0
FunctionEnd

; ------------------------------------------------------------- page behaviour
Function OnDragWindow
	SendMessage $HWNDPARENT ${WM_NCLBUTTONDOWN} ${HTCAPTION} 0
FunctionEnd

Function OnClose
	${If} $Installing = 1
		Return
	${EndIf}
	Quit
FunctionEnd

Function OnGoLicense
	StrCpy $Step 1
	Call ShowStep
	Call UpdateNext
FunctionEnd

Function OnBackWelcome
	StrCpy $Step 0
	Call ShowStep
FunctionEnd

Function OnAcceptToggle
	${If} $AcceptOn = 1
		StrCpy $AcceptOn 0
		${NSD_SetText} $hAccept "☐   J'accepte le contrat de licence"
		SetCtlColors $hAccept ${CLR_MUTED} ${CLR_BG}
	${Else}
		StrCpy $AcceptOn 1
		${NSD_SetText} $hAccept "☑   J'accepte le contrat de licence"
		SetCtlColors $hAccept ${CLR_BRAND} ${CLR_BG}
	${EndIf}
	Call UpdateNext
FunctionEnd

Function OnDesktopToggle
	${If} $DesktopOn = 1
		StrCpy $DesktopOn 0
		${NSD_SetText} $hDesktop "☐   Créer un raccourci sur le bureau"
		SetCtlColors $hDesktop ${CLR_MUTED} ${CLR_BG}
	${Else}
		StrCpy $DesktopOn 1
		${NSD_SetText} $hDesktop "☑   Créer un raccourci sur le bureau"
		SetCtlColors $hDesktop ${CLR_TEXT} ${CLR_BG}
	${EndIf}
FunctionEnd

Function OnLaunchToggle
	${If} $LaunchOn = 1
		StrCpy $LaunchOn 0
		${NSD_SetText} $hLaunch "☐   Lancer CUBE Launcher à la fin de l'installation"
		SetCtlColors $hLaunch ${CLR_MUTED} ${CLR_BG}
	${Else}
		StrCpy $LaunchOn 1
		${NSD_SetText} $hLaunch "☑   Lancer CUBE Launcher à la fin de l'installation"
		SetCtlColors $hLaunch ${CLR_TEXT} ${CLR_BG}
	${EndIf}
FunctionEnd

Function UpdateNext
	${If} $AcceptOn = 1
		EnableWindow $hNext1 1
		SetCtlColors $hNext1 ${CLR_INK} ${CLR_BRAND}
	${Else}
		EnableWindow $hNext1 0
		SetCtlColors $hNext1 ${CLR_MUTED} ${CLR_SURFACE}
	${EndIf}
	System::Call 'user32::InvalidateRect(p $hNext1, p 0, i 1)'
	System::Call 'user32::UpdateWindow(p $hNext1)'
FunctionEnd

Function OnGoOptions
	StrCpy $Step 2
	Call ShowStep
	${NSD_SetFocus} $hPath
FunctionEnd

Function OnBrowse
	nsDialogs::SelectFolderDialog "Dossier d'installation de CUBE Launcher" "$INSTDIR"
	Pop $0
	${If} $0 != error
		StrCpy $INSTDIR $0
		${NSD_SetText} $hPath "$INSTDIR"
	${EndIf}
FunctionEnd

Function OnInstallClick
	${NSD_GetText} $hPath $0
	${If} $0 == ""
		MessageBox MB_ICONEXCLAMATION "Choisis un dossier d'installation."
		Return
	${EndIf}
	StrCpy $INSTDIR $0

	StrCpy $CreateDesktop $DesktopOn
	StrCpy $RunAfterInstall $LaunchOn

	StrCpy $Installing 1
	StrCpy $Step 3
	Call ShowStep

	; The minimum visible time comes from SetProgress holding each stage.
	StrCpy $ProgressValue 3
	StrCpy $ProgressStatus "Préparation du dossier…"
	Call SetProgress

	Call InstallWebView2
	Call InstallCore

	StrCpy $ProgressValue 100
	StrCpy $ProgressStatus "Installation terminée."
	Call SetProgress
	${NSD_SetText} $hFinPath "$INSTDIR"
	Sleep 250
	StrCpy $Installing 0
	StrCpy $Step 4
	Call ShowStep
FunctionEnd

; /P and /UPDATE get the same brand window but no clicks. There is no instfiles page in
; this template, so sections never run on their own: the work has to be started from here.
Function RunUnattended
	${NSD_KillTimer} RunUnattended
	StrCpy $RunAfterInstall 0
	StrCpy $Installing 1

	StrCpy $ProgressValue 3
	StrCpy $ProgressStatus "Préparation du dossier…"
	Call SetProgress

	Call InstallWebView2
	Call InstallCore

	StrCpy $ProgressValue 100
	StrCpy $ProgressStatus "Installation terminée."
	Call SetProgress

	${If} $UpdateMode = 1
		System::Call 'user32::PostMessage(p $HWNDPARENT, i 0x0111, i 1, i 0)'
		Return
	${EndIf}

	${NSD_SetText} $hFinPath "$INSTDIR"
	StrCpy $Installing 0
	StrCpy $Step 4
	Call ShowStep

	${If} $PassiveMode = 1
		Sleep 1200
		System::Call 'user32::PostMessage(p $HWNDPARENT, i 0x0111, i 1, i 0)'
	${EndIf}
FunctionEnd

Function OnFinish
	${If} $RunAfterInstall = 1
		nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" ""
	${EndIf}
	Quit
FunctionEnd

Function SetProgress
	${If} $hProg != 0
		SendMessage $hProg ${PBM_SETPOS} $ProgressValue 0
		${NSD_SetText} $hStatus "$ProgressStatus"
		StrCpy $0 "$ProgressValue %"
		${NSD_SetText} $hPercent "$0"
		; NSIS blocks its own message loop while it installs, so each stage is forced onto
		; screen here and then held: the bar climbs in visible steps instead of jumping to 100.
		System::Call 'user32::UpdateWindow(p $hProg)'
		System::Call 'user32::UpdateWindow(p $hStatus)'
		System::Call 'user32::UpdateWindow(p $hPercent)'
		${If} $ProgressValue < 100
			Sleep ${PROGRESS_HOLD_MS}
		${EndIf}
	${EndIf}
FunctionEnd



; ------------------------------------------------------------------- WebView2
Function InstallWebView2
	${If} ${RunningX64}
		ReadRegStr $4 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
	${Else}
		ReadRegStr $4 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
	${EndIf}
	${If} $4 == ""
		ReadRegStr $4 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
	${EndIf}

	${If} $4 == ""
		${If} $UpdateMode = 1
			Return
		${EndIf}

		StrCpy $ProgressValue 4
		StrCpy $ProgressStatus "Installation du moteur WebView2…"
		Call SetProgress

		!if "${INSTALLWEBVIEW2MODE}" == "downloadBootstrapper"
			Delete "$TEMP\MicrosoftEdgeWebview2Setup.exe"
			DetailPrint "$(webview2Downloading)"
			NSISdl::download "https://go.microsoft.com/fwlink/p/?LinkId=2124703" "$TEMP\MicrosoftEdgeWebview2Setup.exe"
			Pop $0
			${If} $0 != "success"
				DetailPrint "$(webview2DownloadError)"
				Abort "$(webview2AbortError)"
			${EndIf}
			StrCpy $6 "$TEMP\MicrosoftEdgeWebview2Setup.exe"
			Goto install_webview2
		!endif
		!if "${INSTALLWEBVIEW2MODE}" == "embedBootstrapper"
			Delete "$TEMP\MicrosoftEdgeWebview2Setup.exe"
			File "/oname=$TEMP\MicrosoftEdgeWebview2Setup.exe" "${WEBVIEW2BOOTSTRAPPERPATH}"
			DetailPrint "$(installingWebview2)"
			StrCpy $6 "$TEMP\MicrosoftEdgeWebview2Setup.exe"
			Goto install_webview2
		!endif
		!if "${INSTALLWEBVIEW2MODE}" == "offlineInstaller"
			Delete "$TEMP\MicrosoftEdgeWebView2RuntimeInstaller.exe"
			File "/oname=$TEMP\MicrosoftEdgeWebView2RuntimeInstaller.exe" "${WEBVIEW2INSTALLERPATH}"
			DetailPrint "$(installingWebview2)"
			StrCpy $6 "$TEMP\MicrosoftEdgeWebView2RuntimeInstaller.exe"
			Goto install_webview2
		!endif
		Goto webview2_done

		install_webview2:
			DetailPrint "$(installingWebview2)"
			ExecWait "$6 ${WEBVIEW2INSTALLERARGS} /install" $1
			${If} $1 = 0
				DetailPrint "$(webview2InstallSuccess)"
			${Else}
				DetailPrint "$(webview2InstallError)"
				Abort "$(webview2AbortError)"
			${EndIf}
		Goto webview2_done
	${Else}
		!if "${MINIMUMWEBVIEW2VERSION}" != ""
			${VersionCompare} "${MINIMUMWEBVIEW2VERSION}" "$4" $R0
			${If} $R0 = 1
				StrCpy $ProgressValue 4
				StrCpy $ProgressStatus "Mise à jour de WebView2…"
				Call SetProgress
				update_webview:
					DetailPrint "$(installingWebview2)"
					${If} ${RunningX64}
						ReadRegStr $R1 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate" "path"
					${Else}
						ReadRegStr $R1 HKLM "SOFTWARE\Microsoft\EdgeUpdate" "path"
					${EndIf}
					${If} $R1 == ""
						ReadRegStr $R1 HKCU "SOFTWARE\Microsoft\EdgeUpdate" "path"
					${EndIf}
					${If} $R1 != ""
						ExecWait `"$R1" /install appguid=${WEBVIEW2APPGUID}&needsadmin=true` $1
						${If} $1 = 0
							DetailPrint "$(webview2InstallSuccess)"
						${Else}
							MessageBox MB_ICONEXCLAMATION|MB_ABORTRETRYIGNORE "$(webview2InstallError)" IDIGNORE ignore IDRETRY update_webview
							Quit
							ignore:
						${EndIf}
					${EndIf}
			${EndIf}
		!endif
	${EndIf}

	webview2_done:
	StrCpy $ProgressValue 8
	StrCpy $ProgressStatus "Moteur WebView2 prêt."
	Call SetProgress
FunctionEnd

; --------------------------------------------------------------------- install
Function InstallCore
	StrCpy $ProgressValue 12
	StrCpy $ProgressStatus "Copie de CUBE Launcher…"
	Call SetProgress

	SetOutPath $INSTDIR
	!ifmacrodef NSIS_HOOK_PREINSTALL
	!insertmacro NSIS_HOOK_PREINSTALL
	!endif
	!insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
	File "${MAINBINARYSRCPATH}"

	StrCpy $ProgressValue 38
	StrCpy $ProgressStatus "Copie des ressources…"
	Call SetProgress

{{#each resources_dirs}}
	CreateDirectory "$INSTDIR\{{this}}"
{{/each}}
{{#each resources}}
	File /a "/oname={{this.[1]}}" "{{no-escape @key}}"
{{/each}}
{{#each binaries}}
	File /a "/oname={{this}}" "{{no-escape @key}}"
{{/each}}
{{#each file_associations as |association| ~}}
{{#each association.ext as |ext| ~}}
	!insertmacro APP_ASSOCIATE "{{ext}}" "{{or association.name ext}}" "{{association-description association.description ext}}" "$INSTDIR\${MAINBINARYNAME}.exe,0" "Open with ${PRODUCTNAME}" "$INSTDIR\${MAINBINARYNAME}.exe $\"%1$\""
{{/each}}
{{/each}}
{{#each deep_link_protocols as |protocol| ~}}
	WriteRegStr SHCTX "Software\Classes\{{protocol}}" "URL Protocol" ""
	WriteRegStr SHCTX "Software\Classes\{{protocol}}" "" "URL:${BUNDLEID} protocol"
	WriteRegStr SHCTX "Software\Classes\{{protocol}}\DefaultIcon" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
	WriteRegStr SHCTX "Software\Classes\{{protocol}}\shell\open\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
{{/each}}

	StrCpy $ProgressValue 66
	StrCpy $ProgressStatus "Enregistrement de l'installation…"
	Call SetProgress

	WriteUninstaller "$INSTDIR\uninstall.exe"
	WriteRegStr SHCTX "${MANUPRODUCTKEY}" "" $INSTDIR

	ReadRegStr $OldMainBinaryName SHCTX "${UNINSTKEY}" "MainBinaryName"
	${If} $OldMainBinaryName != ""
	${AndIf} $OldMainBinaryName != "${MAINBINARYNAME}.exe"
		Delete "$INSTDIR\$OldMainBinaryName"
	${EndIf}
	WriteRegStr SHCTX "${UNINSTKEY}" "MainBinaryName" "${MAINBINARYNAME}.exe"

	WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "${PRODUCTNAME}"
	WriteRegStr SHCTX "${UNINSTKEY}" "DisplayIcon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\""
	WriteRegStr SHCTX "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
	WriteRegStr SHCTX "${UNINSTKEY}" "Publisher" "${MANUFACTURER}"
	WriteRegStr SHCTX "${UNINSTKEY}" "InstallLocation" "$\"$INSTDIR$\""
	WriteRegStr SHCTX "${UNINSTKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
	WriteRegDWORD SHCTX "${UNINSTKEY}" "NoModify" "1"
	WriteRegDWORD SHCTX "${UNINSTKEY}" "NoRepair" "1"

	${GetSize} "$INSTDIR" "/M=uninstall.exe /S=0K /G=0" $0 $1 $2
	IntOp $0 $0 + ${ESTIMATEDSIZE}
	IntFmt $0 "0x%08X" $0
	WriteRegDWORD SHCTX "${UNINSTKEY}" "EstimatedSize" "$0"
!if "${HOMEPAGE}" != ""
	WriteRegStr SHCTX "${UNINSTKEY}" "URLInfoAbout" "${HOMEPAGE}"
	WriteRegStr SHCTX "${UNINSTKEY}" "URLUpdateInfo" "${HOMEPAGE}"
	WriteRegStr SHCTX "${UNINSTKEY}" "HelpLink" "${HOMEPAGE}"
!endif

	StrCpy $ProgressValue 80
	StrCpy $ProgressStatus "Création des raccourcis…"
	Call SetProgress

	Call CreateOrUpdateStartMenuShortcut
	${If} $CreateDesktop = 1
		Call CreateOrUpdateDesktopShortcut
	${EndIf}

	StrCpy $ProgressValue 94
	StrCpy $ProgressStatus "Vérification…"
	Call SetProgress

!ifmacrodef NSIS_HOOK_POSTINSTALL
	!insertmacro NSIS_HOOK_POSTINSTALL
!endif

	StrCpy $ProgressValue 97
	StrCpy $ProgressStatus "Finalisation…"
	Call SetProgress
FunctionEnd

; ------------------------------------------------------------------- shortcuts
Function CreateOrUpdateStartMenuShortcut
	StrCpy $R0 0
	!insertmacro IsShortcutTarget "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\$OldMainBinaryName"
	Pop $0
	${If} $0 = 1
		!insertmacro SetShortcutTarget "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
		StrCpy $R0 1
	${EndIf}
	!insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\$OldMainBinaryName"
	Pop $0
	${If} $0 = 1
		!insertmacro SetShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
		StrCpy $R0 1
	${EndIf}
	${If} $R0 = 1
		Return
	${EndIf}
	${If} $WixMode = 0
		${If} $UpdateMode = 1
		${OrIf} $NoShortcutMode = 1
			Return
		${EndIf}
	${EndIf}
!if "${STARTMENUFOLDER}" != ""
	CreateDirectory "$SMPROGRAMS\$AppStartMenuFolder"
	CreateShortcut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
	!insertmacro SetLnkAppUserModelId "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
!else
	CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
	!insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
!endif
FunctionEnd

Function CreateOrUpdateDesktopShortcut
	!insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\$OldMainBinaryName"
	Pop $0
	${If} $0 = 1
		!insertmacro SetShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
		Return
	${EndIf}
	${If} $WixMode = 0
		${If} $UpdateMode = 1
		${OrIf} $NoShortcutMode = 1
			Return
		${EndIf}
	${EndIf}
	CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
	!insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
FunctionEnd

Function RestorePreviousInstallLocation
	ReadRegStr $4 SHCTX "${MANUPRODUCTKEY}" ""
	StrCmp $4 "" +2 0
	StrCpy $INSTDIR $4
FunctionEnd

; ------------------------------------------------------------------- uninstall
Var DeleteAppDataCheckbox
Var DeleteAppDataCheckboxState
!define /ifndef WS_EX_LAYOUTRTL 0x00400000

Function un.ConfirmShow
	FindWindow $1 "#32770" "" $HWNDPARENT
	System::Call "user32::GetDpiForWindow(p r1) i .r2"
	${If} $(^RTL) = 1
		StrCpy $3 "${__NSD_CheckBox_EXSTYLE} | ${WS_EX_LAYOUTRTL}"
		IntOp $4 50 * $2
	${Else}
		StrCpy $3 "${__NSD_CheckBox_EXSTYLE}"
		IntOp $4 0 * $2
	${EndIf}
	IntOp $5 100 * $2
	IntOp $6 400 * $2
	IntOp $7 25 * $2
	IntOp $4 $4 / 96
	IntOp $5 $5 / 96
	IntOp $6 $6 / 96
	IntOp $7 $7 / 96
	System::Call 'user32::CreateWindowEx(i r3, w "${__NSD_CheckBox_CLASS}", w "$(deleteAppData)", i ${__NSD_CheckBox_STYLE}, i r4, i r5, i r6, i r7, p r1, i0, i0, i0) i .s'
	Pop $DeleteAppDataCheckbox
	SendMessage $HWNDPARENT ${WM_GETFONT} 0 0 $1
	SendMessage $DeleteAppDataCheckbox ${WM_SETFONT} $1 1
FunctionEnd

Function un.ConfirmLeave
	SendMessage $DeleteAppDataCheckbox ${BM_GETCHECK} 0 0 $DeleteAppDataCheckboxState
FunctionEnd

Function un.onInit
	!insertmacro SetContext
	!insertmacro MUI_UNGETLANGUAGE
	${GetOptions} $CMDLINE "/P" $PassiveMode
	${IfNot} ${Errors}
		StrCpy $PassiveMode 1
	${EndIf}
	${GetOptions} $CMDLINE "/UPDATE" $UpdateMode
	${IfNot} ${Errors}
		StrCpy $UpdateMode 1
	${EndIf}
FunctionEnd

Function un.SkipIfPassive
	${IfThen} $PassiveMode = 1 ${|} Abort ${|}
FunctionEnd

!if "${INSTALLMODE}" != "both"
Function .onInstSuccess
	${If} $PassiveMode = 1
	${OrIf} ${Silent}
		${GetOptions} $CMDLINE "/R" $R0
		${IfNot} ${Errors}
			${GetOptions} $CMDLINE "/ARGS" $R0
			nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" "$R0"
		${EndIf}
	${EndIf}
FunctionEnd
!endif

Section Uninstall
	!ifmacrodef NSIS_HOOK_PREUNINSTALL
	!insertmacro NSIS_HOOK_PREUNINSTALL
	!endif
	!insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"

	Delete "$INSTDIR\${MAINBINARYNAME}.exe"
{{#each resources}}
	Delete "$INSTDIR\{{this.[1]}}"
{{/each}}
{{#each binaries}}
	Delete "$INSTDIR\{{this}}"
{{/each}}
{{#each file_associations as |association| ~}}
{{#each association.ext as |ext| ~}}
	!insertmacro APP_UNASSOCIATE "{{ext}}" "{{or association.name ext}}"
{{/each}}
{{/each}}
{{#each deep_link_protocols as |protocol| ~}}
	ReadRegStr $R7 SHCTX "Software\Classes\{{protocol}}\shell\open\command" ""
	${If} $R7 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
		DeleteRegKey SHCTX "Software\Classes\{{protocol}}"
	${EndIf}
{{/each}}
	Delete "$INSTDIR\AETHER-LICENSE-FR.txt"
	Delete "$INSTDIR\uninstall.exe"
{{#each resources_ancestors}}
	RMDir /REBOOTOK "$INSTDIR\{{this}}"
{{/each}}
	RMDir "$INSTDIR"

	${If} $UpdateMode <> 1
		!insertmacro DeleteAppUserModelId
		!if "${STARTMENUFOLDER}" != ""
			!insertmacro IsShortcutTarget "$SMPROGRAMS\${STARTMENUFOLDER}\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
			Pop $0
			${If} $0 = 1
				!insertmacro UnpinShortcut "$SMPROGRAMS\${STARTMENUFOLDER}\${PRODUCTNAME}.lnk"
				Delete "$SMPROGRAMS\${STARTMENUFOLDER}\${PRODUCTNAME}.lnk"
				RMDir "$SMPROGRAMS\${STARTMENUFOLDER}"
			${EndIf}
		!endif
		!insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
		Pop $0
		${If} $0 = 1
			!insertmacro UnpinShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk"
			Delete "$SMPROGRAMS\${PRODUCTNAME}.lnk"
		${EndIf}
		!insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
		Pop $0
		${If} $0 = 1
			!insertmacro UnpinShortcut "$DESKTOP\${PRODUCTNAME}.lnk"
			Delete "$DESKTOP\${PRODUCTNAME}.lnk"
		${EndIf}
	${EndIf}

!if "${INSTALLMODE}" == "both"
	DeleteRegKey SHCTX "${UNINSTKEY}"
!else if "${INSTALLMODE}" == "perMachine"
	DeleteRegKey HKLM "${UNINSTKEY}"
!else
	DeleteRegKey HKCU "${UNINSTKEY}"
!endif

	${If} $UpdateMode <> 1
		DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
	${EndIf}

	${If} $DeleteAppDataCheckboxState = 1
	${AndIf} $UpdateMode <> 1
		DeleteRegKey SHCTX "${MANUPRODUCTKEY}"
		DeleteRegKey /ifempty SHCTX "${MANUKEY}"
		DeleteRegValue HKCU "${MANUPRODUCTKEY}" "Installer Language"
		DeleteRegKey /ifempty HKCU "${MANUPRODUCTKEY}"
		DeleteRegKey /ifempty HKCU "${MANUKEY}"
		SetShellVarContext current
		RmDir /r "$APPDATA\${BUNDLEID}"
		RmDir /r "$LOCALAPPDATA\${BUNDLEID}"
	${EndIf}

	!ifmacrodef NSIS_HOOK_POSTUNINSTALL
	!insertmacro NSIS_HOOK_POSTUNINSTALL
	!endif

	${If} $PassiveMode = 1
	${OrIf} $UpdateMode = 1
		SetAutoClose true
	${EndIf}
SectionEnd
