<?php

declare(strict_types=1);

namespace Meerkateer;

use InvalidArgumentException;
use RuntimeException;

/** Minimal, dependency-free MKS-1 client for PHP 8.2+ applications. */
final class Client
{
    private string $url;
    private string $key;
    private string $project;
    private string $service;
    private string $environment;
    private $sender;

    public function __construct(string $url, string $key, string $project, string $service, string $environment = 'production', ?callable $sender = null)
    {
        $parts = parse_url($url);
        if ($parts === false || !isset($parts['scheme'], $parts['host']) || !in_array($parts['scheme'], ['http', 'https'], true)
            || isset($parts['user']) || isset($parts['pass']) || isset($parts['query']) || isset($parts['fragment'])
            || (isset($parts['path']) && $parts['path'] !== '' && $parts['path'] !== '/')) {
            throw new InvalidArgumentException('Meerkateer URL must be an HTTP(S) origin without credentials or a path');
        }
        $host = trim($parts['host'], '[]');
        $loopback = $host === 'localhost' || $host === '::1' || (filter_var($host, FILTER_VALIDATE_IP, FILTER_FLAG_IPV4) !== false && str_starts_with($host, '127.'));
        if ($parts['scheme'] !== 'https' && !$loopback) {
            throw new InvalidArgumentException('HTTPS is required except for loopback development URLs');
        }
        if (!preg_match('/^mks_sk_[A-Za-z0-9_-]+$/D', $key)
            || !preg_match('/^[a-z0-9][a-z0-9_-]{0,63}$/D', $project)
            || !preg_match('/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/D', $service)) {
            throw new InvalidArgumentException('Meerkateer service identity is invalid');
        }
        if (!in_array($environment, ['development', 'staging', 'production', 'test', 'local'], true)) {
            throw new InvalidArgumentException('Meerkateer environment is invalid');
        }
        $authority = $parts['host'];
        if (isset($parts['port'])) $authority .= ':' . $parts['port'];
        $this->url = $parts['scheme'] . '://' . $authority . '/';
        $this->key = $key;
        $this->project = $project;
        $this->service = $service;
        $this->environment = $environment;
        $this->sender = $sender;
    }

    public static function fromEnv(): self
    {
        return new self(
            getenv('MEERKATEER_URL') ?: '',
            getenv('MEERKATEER_SERVICE_KEY') ?: '',
            getenv('MEERKATEER_PROJECT') ?: '',
            getenv('MEERKATEER_SERVICE') ?: '',
            getenv('MEERKATEER_ENVIRONMENT') ?: 'production',
        );
    }

    public function heartbeat(string $status = 'ok', string $message = '', ?string $idempotencyKey = null): array
    {
        if (!in_array($status, ['ok', 'degraded', 'down'], true)) throw new InvalidArgumentException('heartbeat status is invalid');
        return $this->send('heartbeat', ['status' => $status, 'message' => $message], $idempotencyKey);
    }

    public function event(string $kind, string $level = 'info', string $message = '', int $count = 1, ?string $idempotencyKey = null): array
    {
        if (strlen($kind) > 128 || !preg_match('/^[a-z0-9]+(_[a-z0-9]+)+$/D', $kind)) throw new InvalidArgumentException('event kind is invalid');
        if (!in_array($level, ['info', 'warning', 'error', 'critical'], true)) throw new InvalidArgumentException('event level is invalid');
        if ($count < 1 || $count > 1000000) throw new InvalidArgumentException('event count is invalid');
        return $this->send('event', ['kind' => $kind, 'level' => $level, 'message' => $message, 'count' => $count], $idempotencyKey);
    }

    public function deploy(string $version, string $commit, string $status = 'finished', ?string $idempotencyKey = null): array
    {
        if ($version === '' || strlen($version) > 128 || $commit === '' || strlen($commit) > 128) throw new InvalidArgumentException('deployment identity is invalid');
        if (!in_array($status, ['started', 'finished', 'failed'], true)) throw new InvalidArgumentException('deployment status is invalid');
        return $this->send('deploy', ['version' => $version, 'commit' => $commit, 'status' => $status], $idempotencyKey);
    }

    private function send(string $kind, array $fields, ?string $id): array
    {
        if (isset($fields['message']) && strlen($fields['message']) > 1024) throw new InvalidArgumentException('message is too long');
        $id ??= self::uuid();
        if (!preg_match('/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iD', $id)) throw new InvalidArgumentException('idempotency key must be a UUID');
        $body = json_encode(array_merge([
            'interface_version' => '1', 'service' => $this->service, 'project' => $this->project,
            'environment' => $this->environment, 'timestamp' => gmdate('Y-m-d\TH:i:s\Z'),
        ], $fields), JSON_THROW_ON_ERROR);
        if (strlen($body) > 16 * 1024) throw new InvalidArgumentException('telemetry payload is too large');
        $endpoint = $this->url . 'v1/ingest/' . $kind;
        $headers = [
            'Authorization: Bearer ' . $this->key,
            'Content-Type: application/json',
            'Accept: application/json',
            'Idempotency-Key: ' . $id,
        ];
        for ($attempt = 0; $attempt <= 2; $attempt++) {
            try {
                [$status, $response] = $this->sender !== null
                    ? ($this->sender)($endpoint, $body, $headers)
                    : self::post($endpoint, $body, $headers);
                if ($status === 200 || $status === 202) {
                    $ack = json_decode($response, true, 512, JSON_THROW_ON_ERROR);
                    if (!is_array($ack) || ($ack['idempotency_key'] ?? null) !== $id) throw new RuntimeException('Meerkateer acknowledgement key mismatch');
                    return $ack;
                }
                if ($status !== 429 && $status < 500) throw new RuntimeException('Meerkateer rejected telemetry with HTTP ' . $status);
            } catch (RuntimeException $error) {
                if (str_starts_with($error->getMessage(), 'Meerkateer rejected')) throw $error;
                if ($attempt === 2) throw new RuntimeException('telemetry delivery failed after bounded retries');
            }
            if ($attempt < 2) usleep(min(250000 * (2 ** $attempt), 1000000));
        }
        throw new RuntimeException('telemetry delivery failed after bounded retries');
    }

    private static function post(string $endpoint, string $body, array $headers): array
    {
        $context = stream_context_create(['http' => [
            'method' => 'POST', 'header' => implode("\r\n", $headers), 'content' => $body,
            'timeout' => 5, 'ignore_errors' => true, 'follow_location' => 0, 'max_redirects' => 0,
        ], 'ssl' => ['verify_peer' => true, 'verify_peer_name' => true]]);
        $stream = @fopen($endpoint, 'rb', false, $context);
        if ($stream === false) throw new RuntimeException('telemetry transport failed');
        $response = stream_get_contents($stream, 64 * 1024 + 1);
        fclose($stream);
        if ($response === false || strlen($response) > 64 * 1024) throw new RuntimeException('Meerkateer acknowledgement is too large');
        $statusLine = $http_response_header[0] ?? '';
        if (!preg_match('/^HTTP\/\S+ ([0-9]{3})/', $statusLine, $match)) throw new RuntimeException('Meerkateer response is invalid');
        return [(int) $match[1], $response];
    }

    private static function uuid(): string
    {
        $bytes = random_bytes(16);
        $bytes[6] = chr((ord($bytes[6]) & 0x0f) | 0x40);
        $bytes[8] = chr((ord($bytes[8]) & 0x3f) | 0x80);
        $hex = bin2hex($bytes);
        return substr($hex, 0, 8) . '-' . substr($hex, 8, 4) . '-' . substr($hex, 12, 4) . '-' . substr($hex, 16, 4) . '-' . substr($hex, 20);
    }
}
