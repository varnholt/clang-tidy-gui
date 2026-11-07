// Try modernize-use-nullptr in analysis mode, then apply its suggested fixes.
int main() {
    int *pointer = 0;
    return pointer != 0;
}
