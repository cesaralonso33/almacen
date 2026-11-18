import assert from 'node:assert/strict';
import test from 'node:test';
import { allSettledConcurrent } from '../src/lib/allSettledConcurrent.js';

test('inicia las cargas a la vez y conserva resultados parciales cuando una falla', async () => {
    let iniciadas = 0;
    let liberarPrimera;

    const primera = () => {
        iniciadas += 1;
        return new Promise((resolve) => {
            liberarPrimera = () => resolve('tablero');
        });
    };
    const segunda = async () => {
        iniciadas += 1;
        return 'pedidos';
    };
    const tercera = async () => {
        iniciadas += 1;
        throw new Error('altas fuera de línea');
    };

    const pendientes = allSettledConcurrent(primera, segunda, tercera);
    await Promise.resolve();

    assert.equal(iniciadas, 3, 'debe iniciar las solicitudes sin esperar una respuesta anterior');
    liberarPrimera();
    const [tablero, pedidos, altas] = await pendientes;

    assert.deepEqual(tablero, { status: 'fulfilled', value: 'tablero' });
    assert.deepEqual(pedidos, { status: 'fulfilled', value: 'pedidos' });
    assert.equal(altas.status, 'rejected');
    assert.match(String(altas.reason), /altas fuera de línea/);
});
