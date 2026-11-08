<?php
declare(strict_types=1);
// Synthetic adapter checks only: no real configuration, order or bank requests.
if (PHP_SAPI !== 'cli' || $argc !== 2) { exit("CLI: php check-support.php ADAPTER_DIRECTORY\n"); }
$source = rtrim($argv[1], '/');
$root = sys_get_temp_dir() . '/dpdf-support-test-' . bin2hex(random_bytes(8));
mkdir($root . '/bs', 0700, true);
mkdir($root . '/dirpay', 0700, true);
try {
    copy($source . '/dpdf-payment.php', $root . '/bs/dpdf-payment.php');
    file_put_contents($root . '/dirpay/lib.php', '<?php function dirpay_hash_ver2(array $fields, string $key): string { return hash_hmac("sha256", json_encode($fields), $key); }');
    define('DPDF_PAYMENT_TEST', true);
    require $root . '/bs/dpdf-payment.php';
    $settings = ['client_id'=>'fixture-client', 'store_key'=>'fixture-only-signing-key'];
    $check = function (bool $ok, string $message): void { if (!$ok) { throw new RuntimeException($message); } };
    foreach (['1'=>'1.00', '10,50'=>'10.50', '9999.99'=>'9999.99', '10000'=>'10000.00', '0.99'=>'invalid', '10000.01'=>'invalid'] as $amount=>$expected) {
        try {
            $fields = dpdf_payment_fields(['amount'=>(string)$amount, 'target'=>'foreign', 'ref'=>'foreign', 'sourceSite'=>'foreign'], $settings);
            $check($expected !== 'invalid' && $fields['amount'] === $expected, 'Amount normalization/bounds');
            $check((bool)preg_match('/^DPDF-[a-f0-9]{24}$/D', $fields['oid']), 'Independent dPDF order namespace');
            $check($fields['orderRef'] === 'dPDF development support', 'Fixed project reference');
            $check($fields['okurl'] === 'https://idz.si/bs/dpdf-return.php' && $fields['failUrl'] === $fields['okurl'], 'Fixed local return URL');
            $check($fields['currency'] === '978' && $fields['clientid'] === 'fixture-client', 'Configured recipient/currency');
            $hash = $fields['hash']; unset($fields['hash']);
            $check(hash_equals(dirpay_hash_ver2($fields,$settings['store_key']),$hash), 'Adapter passes every field to the signing helper');
            $check(str_contains(dpdf_order_path($fields['oid']),'/private/dpdf/DPDF-'), 'Separate private state namespace');
        } catch (InvalidArgumentException $error) { $check($expected === 'invalid','Unexpected validation failure'); }
    }
    foreach (['0', '-1', '1e3', 'NaN', '10.001', '01', '', ['10']] as $amount) {
        try { dpdf_payment_fields(['amount'=>$amount],$settings); throw new RuntimeException('Invalid amount accepted'); }
        catch (InvalidArgumentException $expected) {}
    }
    foreach ([['email'=>'not-an-email'], ['name'=>"name\nheader"], ['name'=>str_repeat('x',161)], ['email'=>['x']]] as $extra) {
        try { dpdf_payment_fields(['amount'=>'10']+$extra,$settings); throw new RuntimeException('Invalid contact accepted'); }
        catch (InvalidArgumentException $expected) {}
    }
    foreach (['../foreign', 'RUWAC-'.str_repeat('a',24), 'DPDF-'.str_repeat('a',23)] as $oid) {
        try { dpdf_order_path($oid); throw new RuntimeException('Foreign order namespace accepted'); }
        catch (InvalidArgumentException $expected) {}
    }
    echo "PASS: synthetic amount/contact validation, dPDF namespace, fixed recipient/routes and signing-helper delegation; no bank request\n";
} finally {
    foreach ([$root.'/bs/dpdf-payment.php',$root.'/dirpay/lib.php'] as $file) { if (is_file($file)) { unlink($file); } }
    foreach ([$root.'/bs',$root.'/dirpay',$root] as $directory) { if (is_dir($directory)) { rmdir($directory); } }
}
