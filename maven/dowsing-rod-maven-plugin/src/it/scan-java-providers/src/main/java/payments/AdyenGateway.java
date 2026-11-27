package payments;

// Adyen payment provider implementation.
public class AdyenGateway implements PaymentGateway {
    private final AdyenClient adyenClient;
    private final AuditLog auditLog;

    public AdyenGateway(AdyenClient adyenClient, AuditLog auditLog) {
        this.adyenClient = adyenClient;
        this.auditLog = auditLog;
    }

    @Override
    public ChargeResult charge(long amount, String currency, String customerId) {
        long validated = Validation.validateAmount(amount);
        ChargeRequest request = Requests.buildChargeRequest(validated, currency, customerId);
        ProviderResponse response = adyenClient.checkout().authorise(request);
        if (!"succeeded".equals(response.getStatus())) {
            throw new PaymentException("Adyen charge failed: " + response.getFailureMessage());
        }
        ChargeResult result = Results.normalizeChargeResult(response);
        auditLog.record("charge", "adyen", result);
        return result;
    }

    @Override
    public RefundResult refund(String transactionId, Long amount) {
        Transaction transaction = Transactions.lookup(transactionId);
        long refundAmount = amount != null ? amount : transaction.getAmount();
        RefundRequest request = Requests.buildRefundRequest(transaction.getProviderId(), refundAmount);
        ProviderResponse response = adyenClient.modifications().refund(request);
        if (!"succeeded".equals(response.getStatus())) {
            throw new PaymentException("Adyen refund failed: " + response.getFailureMessage());
        }
        RefundResult result = Results.normalizeRefundResult(response);
        auditLog.record("refund", "adyen", result);
        return result;
    }
}
