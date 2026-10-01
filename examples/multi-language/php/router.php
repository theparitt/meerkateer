<?php

declare(strict_types=1);

require_once __DIR__ . '/bootstrap.php';

use Meerkateer\Client;

function reply(int $status, array $payload): never
{
    http_response_code($status);
    header('Content-Type: application/json');
    echo json_encode($payload, JSON_THROW_ON_ERROR);
    exit;
}

$method = $_SERVER['REQUEST_METHOD'] ?? '';
$path = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH);
if ($method === 'GET' && $path === '/health') {
    reply(200, ['language' => LANGUAGE, 'status' => readState()]);
}

$routes = [
    '/scenario/healthy' => 'ok',
    '/scenario/degraded' => 'degraded',
    '/scenario/down' => 'down',
];
if ($method !== 'POST' || !is_string($path) || !isset($routes[$path])) {
    reply(404, ['error' => 'not_found']);
}

try {
    signal(Client::fromEnv(), $routes[$path]);
    reply(200, ['language' => LANGUAGE, 'status' => $routes[$path]]);
} catch (Throwable $error) {
    reply(502, ['error' => 'telemetry_failed', 'detail' => $error->getMessage()]);
}
