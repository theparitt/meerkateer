<?php

declare(strict_types=1);

require_once __DIR__ . '/Meerkateer.php';

$client = new Meerkateer\Client('http://127.0.0.1:18081', 'mks_sk_fixture123', 'store', 'worker-01', 'test');
$ack = $client->heartbeat('ok', 'worker ready');
if (($ack['status'] ?? null) !== null || !isset($ack['idempotency_key'])) {
    throw new RuntimeException('missing network acknowledgement');
}
echo "PHP HTTP transport test passed\n";
