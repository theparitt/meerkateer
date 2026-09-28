<?php

declare(strict_types=1);

if ($_SERVER['REQUEST_URI'] !== '/v1/ingest/heartbeat') {
    http_response_code(404);
    exit;
}
$payload = json_decode(file_get_contents('php://input'), true);
if (!is_array($payload) || ($payload['status'] ?? null) !== 'ok') {
    http_response_code(400);
    exit;
}
http_response_code(202);
header('Content-Type: application/json');
echo json_encode(['idempotency_key' => $_SERVER['HTTP_IDEMPOTENCY_KEY'] ?? ''], JSON_THROW_ON_ERROR);
