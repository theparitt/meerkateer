<?php

declare(strict_types=1);

require_once __DIR__ . '/Meerkateer.php';

use Meerkateer\Client;

$calls = [];
$client = new Client('http://127.0.0.1:6510', 'mks_sk_fixture123', 'store', 'worker-01', 'test',
    function (string $url, string $body, array $headers) use (&$calls): array {
        $id = substr(current(array_values(array_filter($headers, fn ($header) => str_starts_with($header, 'Idempotency-Key: ')))), strlen('Idempotency-Key: '));
        $calls[] = [$url, $body, $id];
        if (count($calls) === 1) return [503, 'busy'];
        return [202, json_encode(['idempotency_key' => $id], JSON_THROW_ON_ERROR)];
    });

$client->heartbeat('ok', 'worker ready');
if (count($calls) !== 2 || $calls[0] !== $calls[1] || $calls[0][0] !== 'http://127.0.0.1:6510/v1/ingest/heartbeat') {
    throw new RuntimeException('retry changed the fact identity');
}

try {
    new Client('http://example.com', 'mks_sk_fixture123', 'store', 'worker-01');
    throw new RuntimeException('accepted insecure remote URL');
} catch (InvalidArgumentException) {
    // Expected.
}

echo "PHP SDK tests passed\n";
