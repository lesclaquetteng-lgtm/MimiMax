param(
	[string]$Command = 'list',
	[int]$Port = 0,
	[string]$Password = ''
)

# A minimal RCON client, the console of a hosted server is not attached to a terminal.
# The protocol is four little endian integers framing the payload.

$root = 'C:\Users\ee\Desktop\MimiMax\tools\aether-home\servers\local-test\server.properties'
if ($Port -eq 0 -or -not $Password) {
	foreach ($line in Get-Content $root) {
		if ($line -match '^rcon\.port=(.+)$') { if ($Port -eq 0) { $Port = [int]$Matches[1] } }
		if ($line -match '^rcon\.password=(.+)$') { if (-not $Password) { $Password = $Matches[1] } }
	}
}

function Send-Packet([System.IO.Stream]$stream, [int]$id, [int]$type, [string]$body) {
	$payload = [System.Text.Encoding]::ASCII.GetBytes($body)
	$length = 4 + 4 + $payload.Length + 2
	$buffer = New-Object byte[] (4 + $length)
	[Array]::Copy([BitConverter]::GetBytes([int]$length), 0, $buffer, 0, 4)
	[Array]::Copy([BitConverter]::GetBytes([int]$id), 0, $buffer, 4, 4)
	[Array]::Copy([BitConverter]::GetBytes([int]$type), 0, $buffer, 8, 4)
	[Array]::Copy($payload, 0, $buffer, 12, $payload.Length)
	$stream.Write($buffer, 0, $buffer.Length)
	$stream.Flush()
}

function Read-Packet([System.IO.Stream]$stream) {
	$head = New-Object byte[] 4
	$read = 0
	while ($read -lt 4) {
		$got = $stream.Read($head, $read, 4 - $read)
		if ($got -le 0) { throw 'the server closed the connection' }
		$read += $got
	}
	$length = [BitConverter]::ToInt32($head, 0)
	$body = New-Object byte[] $length
	$read = 0
	while ($read -lt $length) {
		$got = $stream.Read($body, $read, $length - $read)
		if ($got -le 0) { throw 'the server closed the connection' }
		$read += $got
	}
	return [PSCustomObject]@{
		Id = [BitConverter]::ToInt32($body, 0)
		Type = [BitConverter]::ToInt32($body, 4)
		Body = [System.Text.Encoding]::ASCII.GetString($body, 8, $length - 10)
	}
}

$client = New-Object System.Net.Sockets.TcpClient
$client.Connect('127.0.0.1', $Port)
$stream = $client.GetStream()
$stream.ReadTimeout = 5000

Send-Packet $stream 1 3 $Password
$auth = Read-Packet $stream
if ($auth.Id -eq -1) { Write-Error 'the password was refused'; exit 1 }

Send-Packet $stream 2 2 $Command
$answer = Read-Packet $stream
Write-Output $answer.Body
$client.Close()
