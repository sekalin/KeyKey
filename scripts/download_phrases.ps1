$ErrorActionPreference = 'Stop'
$dest = Join-Path $PSScriptRoot "..\data\phrases"
if (!(Test-Path $dest)) {
    New-Item -ItemType Directory -Force -Path $dest | Out-Null
}

$files = @(
    'phrase.general.tsv',
    'phrase.software.tsv',
    'phrase.ai-data-science.tsv',
    'phrase.semiconductor.tsv',
    'phrase.finance.tsv',
    'phrase.network-security.tsv',
    'phrase.business.tsv'
)

$baseUrl = 'https://raw.githubusercontent.com/polobread/KeyKey/master/DataSource/chichi77Collection/'

foreach ($file in $files) {
    $url = $baseUrl + $file
    $out = Join-Path $dest $file
    Write-Host "Downloading $file..."
    Invoke-WebRequest -Uri $url -OutFile $out -UserAgent 'PowerShell'
}

Write-Host "All phrase files downloaded successfully."
Get-ChildItem $dest
