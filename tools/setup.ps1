<#
.SYNOPSIS
	Prepares the toolchain that is needed to build the Aether client.

.DESCRIPTION
	Fabric Loom 1.19 requires a JDK 25 to run Gradle, the game itself is compiled for Java 21.
	This script downloads both, it is safe to run it more than once.
#>
[CmdletBinding()]
param(
	[string]$GradleVersion = '9.8.2',
	[string]$JdkVersion = '25'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$tools = Join-Path $root 'tools'

function Get-ExistingJdk {
	# A Java 21 or newer runtime is enough to run the game.
	$candidates = @(
		'C:\Program Files\Java\jdk-22',
		'C:\Program Files\Java\jdk-21',
		'C:\Program Files\Eclipse Adoptium\jdk-21'
	)
	foreach ($candidate in $candidates) {
		if (Test-Path (Join-Path $candidate 'bin\java.exe')) {
			return $candidate
		}
	}
	return $null
}

Write-Host '== Aether toolchain setup ==' -ForegroundColor Cyan

# --- Gradle -----------------------------------------------------------------
$gradleDir = Join-Path $tools "gradle-$GradleVersion"
if (-not (Test-Path $gradleDir)) {
	$archive = Join-Path $tools "gradle-$GradleVersion-bin.zip"
	Write-Host "Downloading Gradle $GradleVersion…"
	Invoke-WebRequest "https://services.gradle.org/distributions/gradle-$GradleVersion-bin.zip" `
		-OutFile $archive -UseBasicParsing
	Expand-Archive -Path $archive -DestinationPath $tools -Force
} else {
	Write-Host "Gradle $GradleVersion is already installed." -ForegroundColor Green
}

# --- JDK 25 (for Gradle and Loom) -------------------------------------------
$jdkRoot = Join-Path $tools 'jdk25'
if (-not (Test-Path $jdkRoot)) {
	Write-Host "Downloading Temurin JDK $JdkVersion…"
	$archive = Join-Path $tools 'jdk25.zip'
	Invoke-WebRequest "https://api.adoptium.net/v3/binary/latest/$JdkVersion/ga/windows/x64/jdk/hotspot/normal/eclipse" `
		-OutFile $archive -UseBasicParsing
	Expand-Archive -Path $archive -DestinationPath $jdkRoot -Force
} else {
	Write-Host 'JDK 25 is already installed.' -ForegroundColor Green
}

$jdk25 = (Get-ChildItem $jdkRoot -Directory | Select-Object -First 1).FullName

# --- JDK 21+ (to run the game) ----------------------------------------------
$jdk21 = Get-ExistingJdk
if (-not $jdk21) {
	Write-Host 'No Java 21 runtime found, the game needs one.' -ForegroundColor Yellow
} else {
	Write-Host "Java runtime for the game: $jdk21" -ForegroundColor Green
}

Write-Host ''
Write-Host 'Done. Build the client with:' -ForegroundColor Cyan
Write-Host "  `$env:JAVA_HOME = '$jdk25'"
Write-Host "  cd aether-client"
Write-Host "  ..\tools\gradle-$GradleVersion\bin\gradle.bat build"
Write-Host ''
Write-Host 'And the launcher with:' -ForegroundColor Cyan
Write-Host '  cd launcher'
Write-Host '  npm install'
Write-Host '  npm start'
