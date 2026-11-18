import assert from 'node:assert/strict';
import test from 'node:test';
import { SingleFlight } from '../src/lib/singleFlight.js';

test('coalesce llamadas simultáneas y permite reintentar después de error', async () => {
    const flight = new SingleFlight();
    let llamadas = 0;
    let liberar;

    const primera = flight.run(() => {
        llamadas += 1;
        return new Promise((resolve) => {
            liberar = () => resolve('sincronizado');
        });
    });
    const duplicada = flight.run(async () => {
        llamadas += 1;
        return 'no debe ejecutarse';
    });

    assert.strictEqual(duplicada, primera, 'ambos disparadores comparten el mismo trabajo');
    assert.equal(flight.running, true);
    await Promise.resolve();
    assert.equal(llamadas, 1, 'solo se inicia una sincronización');
    liberar();
    assert.equal(await primera, 'sincronizado');
    assert.equal(flight.running, false);

    await assert.rejects(flight.run(async () => {
        llamadas += 1;
        throw new Error('corte de red');
    }), /corte de red/);
    assert.equal(flight.running, false, 'un error libera el coordinador');

    assert.equal(await flight.run(async () => {
        llamadas += 1;
        return 'reintento';
    }), 'reintento');
    assert.equal(llamadas, 3);
});

test('propaga el resultado compartido para no confundir un fallo con éxito', async () => {
    const flight = new SingleFlight();
    let liberar;
    const iniciada = flight.run(() => new Promise((resolve) => {
        liberar = () => resolve(false);
    }));
    const unida = flight.run(async () => true);

    await Promise.resolve();
    liberar();
    assert.equal(await iniciada, false);
    assert.equal(await unida, false);
});
