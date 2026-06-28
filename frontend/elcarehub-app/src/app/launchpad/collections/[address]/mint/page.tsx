import { use } from "react";
import CollectionMintClient from "./CollectionMintClient";

export default function CollectionMintPage({
  params,
}: {
  params: Promise<{ address: string }>;
}) {
  const { address } = use(params);
  return <CollectionMintClient address={address} />;
}
